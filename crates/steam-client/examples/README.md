# steam-client-lib examples

Run each from the repository root. `RUST_LOG=steam_client=info` (or `debug`) shows the client's own log.

The spawned examples show the pattern for a bot: `let (handle, mut events, task) = client.spawn();`, read the
events until they end, react to `Disconnected { reason }` / `Reconnected` (the client reconnects by itself) and to
`LoggedOff` (for good: the task stopped, and it is the last event), call in tasks of their own so that the events
keep being read, and at the end `handle.log_off()` (a second Ctrl-C exits without waiting; a failed log off is only
printed, as the client stops anyway), then `task.await`, which returns once the connection is closed and says why
the client stopped (`Stopped`).

Build them all, without running any:

```sh
cargo build -p steam-client-lib --examples --all-features
```

## Anonymous: no account

These log on anonymously and can run freely.

| Example | What it shows | Run | Variables and files |
|---|---|---|---|
| `anonymous_logon` | Connect, log on anonymously, spawn, idle past Steam's timeout (the task sends the heartbeats), call a service method, log off. Verified live. | `cargo run -p steam-client-lib --example anonymous_logon` | `IDLE_SECONDS` (210) |
| `events` | Everything Steam pushes: each event's EMsg, and the method of each service notification, to know what to match and parse. | `cargo run -p steam-client-lib --example events` | `EVENT_SECONDS` (30) |
| `service_call` | A non-authed service call before logon, and calling a method the crate doesn't know, through a marker type of six lines (`impl ServiceMethod`). | `cargo run -p steam-client-lib --example service_call` | none |
| `reconnect` | A bot's lifecycle: `Disconnected { reason }`, `Reconnected`, `Lagged` and `LoggedOff`, calls made in tasks of their own and failing with `Dropped` while the connection is down, stopping on Ctrl-C (a second one exits at once), `task.await`. | `cargo run -p steam-client-lib --example reconnect` | `STOP=drop` (stop by dropping the handle, which logs off, instead of `log_off`), `CALL_SECONDS` (60) |
| `task_free` | No task: the `next_event()` loop in a `select!` with the bot's own work, which keeps sending the heartbeats. | `cargo run -p steam-client-lib --example task_free` | `RUN_SECONDS` (210), `CALL_SECONDS` (60) |
| `many_accounts` | N anonymous clients on one runtime, and the memory (RSS) each one takes; every client logs off, even if one fails to. | `cargo run -p steam-client-lib --example many_accounts -- 5` | the number of clients as the first argument, or `ACCOUNTS` (5, from 1 to 20); `IDLE_SECONDS` (60) |

## Account: logs on to a Steam account

These log on to a real account. Run `login` first: the others log on with the refresh token it saves. The files
default to `~/.steam-test/`; each variable points elsewhere.

| Example | What it shows | Run | Variables and files |
|---|---|---|---|
| `login` | The first logon with the account name, password and Steam Guard (a device code from the maFile, or a code from stdin), through the two-phase `AuthSession`; saves the refresh token. Later runs log on with it. No steam-mobile. | `STEAM_ACCOUNT=<name> cargo run -p steam-client-lib --example login` | `STEAM_ACCOUNT`; `STEAM_PASSWORD_FILE` (`password`, only without a saved token); `STEAM_MAFILE` (optional); `STEAM_REFRESH_TOKEN_FILE` (`refresh_token`, written) |
| `chat_bot` | An echo bot: spawn, `set_persona_state(Online)`, answer friends' messages, each reply in its own task; skips its own echoes and typing notifications. | `STEAM_ACCOUNT=<name> cargo run -p steam-client-lib --example chat_bot` | `STEAM_ACCOUNT`; `STEAM_REFRESH_TOKEN_FILE` |
| `trade_notifications` | When trade offers change, from `ClientUserNotifications` or `SteamNotificationClient.NotificationsReceived#1`: a trigger to fetch the offers with steam-trading. | `STEAM_ACCOUNT=<name> cargo run -p steam-client-lib --example trade_notifications` | `STEAM_ACCOUNT`; `STEAM_REFRESH_TOKEN_FILE` |
| `cs2_inspect` | CS2 inspect links over its game coordinator: spawn, `Cs2::connect` (plays CS2, hello until welcome), then `inspect_str` for each link (embedded ones offline), printing the def index, paint index, pattern, float, stickers and keychains; one request at a time, 1.1 s apart; Ctrl-C logs off. The account must own CS2. | `STEAM_ACCOUNT=<name> cargo run -p steam-client-lib --features cs2 --example cs2_inspect -- '<link>' ...` | `STEAM_ACCOUNT`; `STEAM_REFRESH_TOKEN_FILE`; the links as arguments, or stdin lines |
| `cs2_profile` | CS2 profiles, matches and the inventory: `Cs2::connect`, the account's inventory (from the welcome) and `request_matchmaking_stats`, then for each argument `player_profile` + `recent_matches` (a SteamID) or `match_info` (a share code), then `next_event` until Ctrl-C (session lost / welcomed again, stats, items acquired / changed / removed, other messages). The account must own CS2; profiles of its friends only. | `STEAM_ACCOUNT=<name> cargo run -p steam-client-lib --features cs2 --example cs2_profile -- <steamid64> <share code> ...` | `STEAM_ACCOUNT`; `STEAM_REFRESH_TOKEN_FILE`; SteamIDs and share codes as arguments |

## Safety

- Ask before running an account example against an account that matters: each one calls Steam's authentication
  and logs the account on. The first `login` adds a "SteamHelper-rs" device to the account's authorized devices.
- Refresh tokens are saved readable by the owner only (mode 0600). A token logs on to the account without the
  password for about 200 days: keep the file private, and delete it to log in again from scratch.
- Nothing secret is printed: no password, Steam Guard code or token, only their expiries.
- Not verified live: every account example. `chat_bot` and `trade_notifications` also rest on assumptions about
  what Steam sends (the new chat with an online persona; which of the two notifications Steam uses); their headers
  say which.
