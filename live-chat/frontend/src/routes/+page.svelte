<script lang="ts">
	interface ConnectionProps {
		areElementsDisabled: boolean;
		connectionButtonBg: string;
		connectionButtonLabel: string;
		messageFieldPlaceholder: string;
	}

	const connect: ConnectionProps = {
		areElementsDisabled: true,
		connectionButtonBg: 'bg-green-500 hover:bg-green-600',
		connectionButtonLabel: 'Connect',
		messageFieldPlaceholder: "Click on 'Connect' to start a conversation"
	};
	const disconect: ConnectionProps = {
		areElementsDisabled: false,
		connectionButtonBg: 'bg-red-500 hover:bg-red-600',
		connectionButtonLabel: 'Disconnect',
		messageFieldPlaceholder: 'Connected! Type your message'
	};
	const waiting: ConnectionProps = {
		areElementsDisabled: true,
		connectionButtonBg: 'bg-blue-700 hover:bg-blue-800',
		connectionButtonLabel: 'Waiting',
		messageFieldPlaceholder: 'Waiting for someone to show up...'
	};

	let chatHistory: { type: 'received' | 'sent'; text: string }[] = $state([]);
	let connectionProps: ConnectionProps = $state(connect);
	let messageFieldValue = $state('');
	let ws: WebSocket | undefined = undefined;

	const connection = () => {
		if (ws === undefined) {
			ws = new WebSocket('wss://localhost:9000/chat');
			ws.addEventListener('close', () => {
				connectionProps = connect;
				ws = undefined;
			});
			ws.addEventListener('message', (event) => {
				if (event.data == 'OK') {
					connectionProps = disconect;
					return;
				}
				chatHistory.push({ type: 'received', text: event.data });
			});
			ws.addEventListener('open', () => {
				connectionProps = waiting;
			});
		} else {
			ws.close();
			connectionProps = connect;
		}
	};

	const send = () => {
		if (ws === undefined || messageFieldValue === '') {
			return;
		}
		ws.send(messageFieldValue);
		chatHistory.push({ type: 'sent', text: messageFieldValue });
		messageFieldValue = '';
	};
</script>

<svelte:head>
	<title>Live Chat</title>
</svelte:head>

<div class="bg-gray-100 flex flex-col h-screen w-screen">
	<div class="text-center text-gray-500 text-sm py-2">
		📝
		<a
			class="duration-200 hover:text-blue-700 text-blue-900 transition underline"
			href="https://c410-f3r.github.io/thoughts/building-a-real-time-chat-using-web-sockets-over-http2-streams"
		>
			Building a real-time chat using WebSockets over HTTP/2 streams
		</a>
	</div>
	<div
		class="bg-white border border-gray-300 flex flex-col grow max-w-4xl mx-auto shadow-lg w-full"
	>
		<div class="h-full overflow-y-auto p-4 space-y-2">
			{#each chatHistory as chat}
				{#if chat.type === 'received'}
					{@render receivedMessage(chat.text)}
				{:else}
					{@render sentMessage(chat.text)}
				{/if}
			{/each}
		</div>
		<div class="bg-gray-50 border-t border-gray-300 p-4">
			<div class="flex-col gap-4 sm:hidden items-center space-y-4">
				{@render messageField()}
				<div class="flex gap-4">
					{@render connectionButton()}
					{@render sendButton()}
				</div>
			</div>

			<div class="sm:flex flex-row gap-4 hidden items-center">
				{@render connectionButton()}
				{@render messageField()}
				{@render sendButton()}
			</div>
		</div>
	</div>
</div>

{#snippet connectionButton()}
	<button
		class="{connectionProps.connectionButtonBg} cursor-pointer px-4 py-2 rounded-lg text-white transition sm:w-auto w-full"
		onclick={connection}>{connectionProps.connectionButtonLabel}</button
	>
{/snippet}

{#snippet messageField()}
	<input
		class="border border-gray-300 disabled:cursor-not-allowed grow p-2 rounded-lg w-full"
		disabled={connectionProps.areElementsDisabled}
		onkeydown={(e) => {
			if (e.key === 'Enter') {
				send();
			}
		}}
		placeholder={connectionProps.messageFieldPlaceholder}
		type="text"
		bind:value={messageFieldValue}
	/>
{/snippet}

{#snippet receivedMessage(message: string)}
	<div class="flex justify-start">
		<div class="bg-gray-200 max-w-xs px-4 py-2 rounded-lg text-black">
			{message}
		</div>
	</div>
{/snippet}

{#snippet sendButton()}
	<button
		class="bg-blue-500 enabled:hover:bg-blue-600 disabled:cursor-not-allowed cursor-pointer disabled:opacity-50 px-4 py-2 rounded-lg text-white transition sm:w-auto w-full"
		disabled={connectionProps.areElementsDisabled}
		onclick={send}>Send</button
	>
{/snippet}

{#snippet sentMessage(message: string)}
	<div class="flex justify-end">
		<div class="bg-blue-500 max-w-xs px-4 py-2 rounded-lg text-white">
			{message}
		</div>
	</div>
{/snippet}
