# Discord channel setup

Create a Discord application and bot in the Discord Developer Portal, enable only the gateway intents required by the conversations you intend to route, and install the bot with permission to view channels, read message history, send messages, attach files, and send typing indicators.

The production gateway reads the bot token from a named environment variable. The token value is never accepted as a command argument or written to the cursor, staging area, logs, or diagnostic output. The profile and session arguments are operator-owned routing state; they are not embedded in Discord messages.

```sh
export DISCORD_BOT_TOKEN='the-bot-token'
bin/channel-gateway \
  --socket "$KEITH_DATA_ROOT/agentd.sock" \
  --discord-token-env DISCORD_BOT_TOKEN \
  --discord-bot-user-id BOT_USER_ID \
  --discord-intents INTENT_BITSET \
  --discord-profile-id PROFILE_ID \
  --discord-session-id SESSION_ID \
  --discord-cursor "$KEITH_DATA_ROOT/channels/discord-cursor.json" \
  --attachment-root "$KEITH_DATA_ROOT/channel-staging"
```

Run the gateway under the same user account as `agentd`. The worker imports inbound Discord CDN attachments from the bounded staging area into the owning root-tree artifact store before the prompt is submitted. Outbound artifact bytes take the reverse path and are checked by size and SHA-256 before the adapter sends them. Staging tokens are random local identifiers; platform URLs and arbitrary filesystem paths never cross the worker boundary.

The cursor is persisted only after a message has been staged and accepted through `AgentConnection`. A restart therefore replays uncommitted messages, while the stable command ID and message deduplication window prevent a transport retry from silently creating a second action. Replies and scheduled results use the transactional delivery outbox, Discord nonce enforcement, platform receipts, and classified bounded retries.

Use one gateway process per Discord bot account. The adapter rejects an outbound route whose external account does not match its configured bot identity. Stop the gateway before removing its bot token or channel staging directory.

After setup, verify an inbound mention or direct message, a reply, a scheduled return, an attachment in each direction, a gateway reconnect, and a denied conversation that has no authorized route. Inspect safe adapter errors and delivery receipts; never print the token while troubleshooting.
