import * as Schema from "effect/Schema";

const Token = Schema.String.check(Schema.isPattern(/^[a-fA-F0-9]{32,1024}$/));
export const IosNotificationRegistration = Schema.Struct({
  deviceId: Schema.String.check(Schema.isPattern(/^[a-zA-Z0-9_-]{1,128}$/)),
  bundleId: Schema.String.check(Schema.isPattern(/^[a-zA-Z0-9.-]{1,255}$/)),
  apsEnvironment: Schema.Literals(["sandbox", "production"]),
  pushToken: Schema.optionalKey(Token),
  activityToken: Schema.optionalKey(Token),
  notificationsEnabled: Schema.Boolean,
  liveActivitiesEnabled: Schema.Boolean,
});
export type IosNotificationRegistration = typeof IosNotificationRegistration.Type;
export const IosNotificationRegistrationResult = Schema.Struct({
  configured: Schema.Boolean,
});
