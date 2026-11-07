APNs delivery failures

`400 BadDeviceToken`, `DeviceTokenNotForTopic`, `TopicDisallowed`, `Unregistered` (also as `403`) and `410` retire the exact token through a `DeviceDropped` event. Other tokens and what is waiting are unchanged. Apple documents these answers at
https://developer.apple.com/documentation/usernotifications/handling-notification-responses-from-apns

Any other answer (`403 ExpiredProviderToken`, `429`, `5xx`) keeps the token and is logged as `push: not delivered`. The push is not sent again: the item stays in the book and the badge, and the phone catches up on connect.

Each curl delivery permits at most two retries for curl's transient-error classification, with 10-second per-attempt and 20-second retry-window limits. The provider token is reused for 40 minutes. A copy of a log sends nothing.

There is no per-token circuit or backoff in the Rust hub, unlike the old one. What the gate lets through is a few pushes an hour, so it has not been needed. Add one if `push: not delivered` shows otherwise.

Tests use a recording carrier and no credentials. `test/e2e/push/` sends real pushes to a simulator only.
