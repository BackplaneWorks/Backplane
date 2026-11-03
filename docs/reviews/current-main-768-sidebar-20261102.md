# Current main sidebar input review

Reviewed source 61752742fd06dd5596c5aae07d65e0b087727f24, tree 95ccea12da95562d42552caeb353d25b79feecc0. The existing roots, bridges, scanner, reduced input keys and draft filters remain unchanged.

The added board catalog, path-record and RPC-order modules read explicit metadata and existing request payloads. Changed client/presentation code keeps destination-bound drafts, scopes directory replies and preserves account/session authority. Compact actor rendering changes prepared presentation only. The negotiated response omits only the derived board-catalog cache, preserving drafts, preferences and origin. Viewer changes bound metadata/options; mechanical, translation and release changes add no draft readers.

Actual full-output test: work w-t1793604413-1188217-1295302, all 24 cases passed. All 225 executed source/test/guard files matched this checkout; pinned compiler and 66 resources were verified before and after execution. Output SHA-256 cd83cd83cb66e6c0ea17d490d0baf8502c2b965f711f5e731bbd391a9cd5612c. Root refreshed only the three added and eight changed dependency hashes after static review and that actual output. No global proof, application build or live acceptance is claimed by this review.
