#![no_std]
#![no_main]

extern crate alloc;
extern crate esp_backtrace;

use core::net::Ipv4Addr;
use embassy_executor::Spawner;
use embassy_net::{tcp::TcpSocket, DhcpConfig, Ipv4Address, Stack, StackResources};
use embedded_dht_rs::dht22::Dht22;
use embedded_tls::{
  Aes128GcmSha256, Certificate, TlsConfig, TlsConnection, TlsContext, UnsecureProvider,
};
use esp_hal::{
  delay::Delay,
  gpio::{Io, Level, OutputOpenDrain, Pull},
  peripherals::{GPIO, IO_MUX, RADIO_CLK, TIMG1, WIFI},
  prelude::*,
  rng::Rng,
  timer::timg::TimerGroup,
};
use esp_wifi::{
  wifi::{self, AuthMethod, WifiStaDevice},
  EspWifiInitFor,
};
use rand::{rngs::StdRng, SeedableRng};
use rustls_pemfile::Item;
use static_cell::StaticCell;
use wtx::{
  database::{
    client::postgres::{Executor, ExecutorBuffer},
    Executor as _,
  },
  misc::{Uri, Xorshift64},
};

static CA: &[u8] = include_bytes!("../../.certs/root-ca.crt");
static RESOURCES: StaticCell<StackResources<4>> = StaticCell::new();
static STACK: StaticCell<Stack<WifiDevice>> = StaticCell::new();

type WifiDevice = esp_wifi::wifi::WifiDevice<'static, WifiStaDevice>;

#[esp_hal_embassy::main]
async fn main(spawner: Spawner) {
  let uri_str = env!("URI");
  let wifi_pw = env!("WIFI_PW");
  let wifi_ssid = env!("WIFI_SSID");

  let delay = Delay::new();
  let peripherals = esp_hal::init(esp_hal::Config::default());
  let rng = Rng::new(peripherals.RNG);
  let (rand_seed, stack_seed, xorshift64_seed) = seeds(rng);

  esp_alloc::heap_allocator!(64 * 1024);
  esp_hal_embassy::init(TimerGroup::new(peripherals.TIMG0).timer0);

  let wifi_device = wifi_device(
    delay,
    wifi_pw,
    peripherals.RADIO_CLK,
    rng,
    wifi_ssid,
    peripherals.TIMG1,
    peripherals.WIFI,
  )
  .await;

  let mut rx_buffer_plain = [0; 2048];
  let mut rx_buffer_tls = [0; 8192];
  let mut tx_buffer_plain = [0; 2048];
  let mut tx_buffer_tls = [0; 8192];

  let wifi_device_stack = wifi_device_configuration(spawner, stack_seed, wifi_device).await;
  let mut dht22 = dht22(delay, peripherals.GPIO, peripherals.IO_MUX);
  let mut executor = executor(
    rand_seed,
    &mut rx_buffer_plain,
    &mut rx_buffer_tls,
    &mut tx_buffer_plain,
    &mut tx_buffer_tls,
    uri_str,
    xorshift64_seed,
    &wifi_device_stack,
  )
  .await;

  loop {
    delay.delay(2000.millis());
    let sensor_reading = dht22.read().unwrap();
    let _ = executor
      .execute_with_stmt(
        "INSERT INTO sensor (humidity, temperature) VALUES ($1, $2)",
        (sensor_reading.humidity, sensor_reading.temperature),
      )
      .await
      .unwrap();
  }
}

fn dht22(delay: Delay, gpio: GPIO, io_mux: IO_MUX) -> Dht22<OutputOpenDrain<'static>, Delay> {
  let io = Io::new(gpio, io_mux);
  let ood = OutputOpenDrain::new(io.pins.gpio32, Level::High, Pull::None);
  Dht22::new(ood, delay)
}

async fn executor<'plain, 'tls, 'wifi>(
  rand_seed: [u8; 32],
  rx_buffer_plain: &'plain mut [u8; 2048],
  rx_buffer_tls: &'tls mut [u8; 8192],
  tx_buffer_plain: &'plain mut [u8; 2048],
  tx_buffer_tls: &'tls mut [u8; 8192],
  uri_str: &str,
  xorshift64_seed: u64,
  wifi_device_stack: &'wifi Stack<WifiDevice>,
) -> Executor<wtx::Error, ExecutorBuffer, TlsConnection<'tls, TcpSocket<'plain>, Aes128GcmSha256>>
where
  'plain: 'tls,
  'wifi: 'plain,
{
  let Some((Item::X509Certificate(ca), _)) = rustls_pemfile::read_one_from_slice(CA).unwrap()
  else {
    panic!();
  };
  let uri = Uri::new(uri_str);
  let mut socket = TcpSocket::new(wifi_device_stack, rx_buffer_plain, tx_buffer_plain);
  let ipv4_addr: Ipv4Addr = uri.hostname().parse().unwrap();
  let [a, b, c, d] = ipv4_addr.octets();
  socket.connect((Ipv4Address::new(a, b, c, d), uri.port().unwrap())).await.unwrap();
  let mut xorshift64 = Xorshift64::from(xorshift64_seed);
  let executor = Executor::<wtx::Error, _, _>::connect_encrypted(
    &wtx::database::client::postgres::Config::from_uri(&uri).unwrap(),
    ExecutorBuffer::new(usize::MAX, &mut xorshift64),
    &mut xorshift64,
    socket,
    |stream| async {
      let config = TlsConfig::new().with_ca(Certificate::X509(ca.as_ref())).enable_rsa_signatures();
      let mut tls = TlsConnection::new(stream, rx_buffer_tls, tx_buffer_tls);
      tls
        .open(TlsContext::new(
          &config,
          UnsecureProvider::new::<Aes128GcmSha256>(StdRng::from_seed(rand_seed)),
        ))
        .await
        .unwrap();
      Ok(tls)
    },
  )
  .await
  .unwrap();
  executor
}

fn seeds(mut rng: Rng) -> ([u8; 32], u64, u64) {
  let rand_seed = {
    let [_0, _1, _2, _3] = rng.random().to_ne_bytes();
    let [_4, _5, _6, _7] = rng.random().to_ne_bytes();
    let [_8, _9, _10, _11] = rng.random().to_ne_bytes();
    let [_12, _13, _14, _15] = rng.random().to_ne_bytes();
    let [_16, _17, _18, _19] = rng.random().to_ne_bytes();
    let [_20, _21, _22, _23] = rng.random().to_ne_bytes();
    let [_24, _25, _26, _27] = rng.random().to_ne_bytes();
    let [_28, _29, _30, _31] = rng.random().to_ne_bytes();
    [
      _0, _1, _2, _3, _4, _5, _6, _7, _8, _9, _10, _11, _12, _13, _14, _15, _16, _17, _18, _19,
      _20, _21, _22, _23, _24, _25, _26, _27, _28, _29, _30, _31,
    ]
  };
  let stack_seed = {
    let [_0, _1, _2, _3] = rng.random().to_ne_bytes();
    let [_4, _5, _6, _7] = rng.random().to_ne_bytes();
    u64::from_ne_bytes([_0, _1, _2, _3, _4, _5, _6, _7])
  };
  let xorshift64_seed = {
    let [_0, _1, _2, _3] = rng.random().to_ne_bytes();
    let [_4, _5, _6, _7] = rng.random().to_ne_bytes();
    u64::from_ne_bytes([_0, _1, _2, _3, _4, _5, _6, _7])
  };
  (rand_seed, stack_seed, xorshift64_seed)
}

async fn wifi_device(
  delay: Delay,
  pw: &str,
  radio_clk: RADIO_CLK,
  rng: Rng,
  ssid: &str,
  timg1: TIMG1,
  wifi: WIFI,
) -> WifiDevice {
  let timer0 = TimerGroup::new(timg1).timer0;
  let init = esp_wifi::init(EspWifiInitFor::Wifi, timer0, rng, radio_clk).unwrap();
  let config = wifi::ClientConfiguration {
    ssid: ssid.try_into().unwrap(),
    bssid: None,
    auth_method: AuthMethod::WPA2Personal,
    password: pw.try_into().unwrap(),
    channel: None,
  };
  let (rslt, mut controller) = wifi::new_with_config::<WifiStaDevice>(&init, wifi, config).unwrap();
  controller.start().await.unwrap();
  delay.delay(1000.millis());
  controller.connect().await.unwrap();
  rslt
}

async fn wifi_device_configuration(
  spawner: Spawner,
  stack_seed: u64,
  wifi_device: WifiDevice,
) -> &'static Stack<WifiDevice> {
  let wifi_device_stack = &*STACK.init(Stack::new(
    wifi_device,
    embassy_net::Config::dhcpv4({
      let mut config = DhcpConfig::default();
      config.hostname = Some("esp32-postgres".try_into().unwrap());
      config
    }),
    RESOURCES.init(StackResources::<4>::new()),
    stack_seed,
  ));
  spawner.spawn(wifi_runner(wifi_device_stack)).unwrap();
  wifi_device_stack.wait_config_up().await;
  wifi_device_stack.config_v4().unwrap();
  wifi_device_stack
}

#[embassy_executor::task]
async fn wifi_runner(wifi_device: &'static Stack<WifiDevice>) -> ! {
  wifi_device.run().await
}
