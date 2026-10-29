# Privacy policy

Kids Free Games has no ads, no in-app purchases, no third-party analytics and no tracking across other apps or websites.

## What the app sends to our server

The app syncs a child's progress between their devices and powers the leaderboards. It does this without an email, password or real identity. To do so it sends this to our server (a Cloudflare Worker with a D1 database):

- A random player ID and an access token generated on the device.
- The player name (letters only, up to 12) and avatar the player picked.
- The device type (iPhone or iPad, Android, Web or Computer).
- Saved progress: levels, stars and settings.
- Best scores per game.
- Play sessions: which game, when it started and how many seconds it was played.

Player name, avatar and scores are visible to other players on the leaderboards. Nothing else is.

Cloudflare, which hosts the server, sees the IP address of each request as part of normal operation. We do not use it to identify anyone.

## What we do not collect

No email, phone number, real name, location, contacts, photos, microphone or camera data, advertising identifier or browsing history. No data is sold or shared with advertisers or data brokers.

## Controls

- The app works offline. Without a connection, progress stays on the device and syncs later.
- Deleting a player in the profile screen erases that player and all their server data (profile, saves, scores and play sessions).
- To ask for data to be removed or to ask a question, write to kieran@apexstrata.ie.

The server source is in `server/`.
