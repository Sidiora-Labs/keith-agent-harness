# Discord channel setup

Create a Discord application and bot in the Discord Developer Portal, enable only the gateway intents required by the conversations you intend to route, and install the bot into the relevant server with permission to view channels, read message history, send messages, attach files, and send typing indicators.

Store the bot token in Keith's encrypted credential store under the Discord channel owner. Do not place the token in configuration files, command-line arguments, URLs, logs, or diagnostic bundles. The adapter accepts the resolved secret in memory and authenticates REST and Gateway connections without exposing it through its inspection types.

Configure `DiscordConfig::production` with the bot's public user identity and the selected intent bitset. Keep the default Discord API and Gateway endpoints unless a controlled protocol-compatible endpoint is being used for testing. Persist `DiscordCursor` with channel state so reconnects resume the Gateway sequence and retain the bounded recent-message deduplication window.

Bind Discord conversations to profiles through the explicit channel routing service. Missing or ambiguous routes must remain denied; do not embed profile or session routing identifiers in Discord messages. Replies and scheduled results should enter the transactional delivery outbox and use the stable delivery key as the Discord nonce. Stage attachment bytes with `stage_artifact` before claiming the outbound delivery.

After setup, verify an inbound mention or direct message, a reply, a scheduled return, an attachment in each direction, a gateway reconnect, and a denied conversation that has no authorized route. Inspect safe adapter errors and delivery receipts; never print the token while troubleshooting.
