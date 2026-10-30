APNs delivery failures

`400 TopicDisallowed`, `400 DeviceTokenNotForTopic`, `400 BadDeviceToken`,
`400 Unregistered` and `410` retire the exact registered token through the
existing `device.drop` event. History stays available; other device tokens,
source messages and pending approvals are unchanged. Apple documents
TopicDisallowed as a refusal to push to that topic:
https://developer.apple.com/documentation/usernotifications/handling-notification-responses-from-apns

Each process keeps at most 1,024 opaque token/topic/environment/registration
circuits, with at most one activity or four alert requests in flight per
circuit. Only hashes, counts and deadlines are retained, never payloads,
credentials or canonical snapshots. A new device registration timestamp or
topic receives an independent circuit. Permanent refusals are not evicted to
make room; exhausted capacity fails closed and reports the delivery hold once.

Activity updates describe replaceable current state, so overlapping updates
coalesce and successful updates are limited to one per second. Failures back
off for 30, 60, 120, 240 and 480 seconds; six failed delivery jobs open the
circuit until at least 30 minutes from the first failure and 15 minutes from
the last. New demands during a hold do not start a timer, process or queue.
An alert's delivery hold is logged once per hold state; its source notice
remains available in the app. A late old success cannot clear a newer failure.

Each curl delivery, including a webhook delivery, permits at most two retries
for curl's transient-error classification, with its exponential delays,
10-second per-attempt and 20-second retry-window limits. Permanent 400/403
responses are not blanket-retried. The circuit counts delivery jobs, so its
six-job window permits at most 18 HTTP attempts. Process restart clears
transient circuits; retired APNs registrations stay retired in the event log.
Tests use synthetic endpoints and no credentials or real notifications.
