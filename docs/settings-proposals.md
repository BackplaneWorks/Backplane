# Assistant settings proposals

`settings_inspect` returns the current hub's effective provider fallback
switch, ordered hierarchy, and available provider models. It does not expose
account bindings or credentials. An unset switch is off; an unset hierarchy
is empty.

`settings_propose` accepts `on` (`on` or `off`) and `chain` (a comma-separated
list of installed `provider/model` entries). It opens an approval in the
requesting thread. The card shows the old and proposed values, hub-wide
scope, expiry and consequences. A proposal expires ten minutes after it is
created. Only the person's one-time approval applies it; rejecting it or
choosing a generic session permission does not. There is no agent-facing
apply or approval tool.

The hub serializes the decision with other settings writes. A write to either
fallback setting closes other pending fallback proposals as stale, even if a
later write restores the original value. The decision rechecks the snapshot
and current model capabilities. A closed or expired proposal cannot be
replayed. The ask, its exact proposal and the decision are recorded in the
event history. Only changed settings are written, so an unchanged implicit
default remains implicit.

Applying a proposal does not itself start, stop or resume a conversation.
When fallback is enabled, subsequent provider-limit handling can switch
active threads and bots to a later eligible model or hold work if the chain
is exhausted. Existing explicit provider selections and account bindings are
not changed by the proposal itself. Settings outside the fallback switch and
hierarchy, including credentials and default providers, are not supported by
these tools. Never put secrets in a proposal.
