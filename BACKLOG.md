# Backlog

Known edge cases, deferred fixes, and design notes. Each entry records what was found,
the current behaviour, why it was deferred, and what a future fix would look like.

---

## `crates/jira`

### fields.rs

#### FIELDS-1 — Empty string field path silently returns empty object
**Found:** review session 2026-06-09  
**Trigger:** `--fields ","` or `--fields "summary,"` → `split(',')` produces `""` entries  
**Current behaviour:** empty string becomes a key lookup for `""` in the JSON object; never matches; silently omitted → caller gets `{}` with no indication something went wrong  
**Acceptable?** Barely. An LLM won't pass `--fields ","` intentionally, but a trailing comma is plausible.  
**Future fix:** strip empty segments after split in `main.rs` (already `trim`-ed, add `filter(|s| !s.is_empty())`); or validate in `filter_fields` and surface an error.

---

#### FIELDS-2 — All requested fields missing → silent empty object `{}`
**Found:** review session 2026-06-09  
**Trigger:** `--fields nonexistent` on any response  
**Current behaviour:** returns `{}` — structurally valid JSON, but the caller has no idea whether the field doesn't exist or the response was empty.  
**Acceptable?** Yes for now. An LLM can detect `{}` and re-query without `--fields`. A future improvement could warn to stderr when the filtered result is empty.  
**Future fix:** if `fields` non-empty and filtered result is `{}`, print a warning to stderr listing the requested paths.

---

#### FIELDS-3 — Intermediate null on nested path returns null, not omitted
**Found:** review session 2026-06-09  
**Trigger:** `--fields status.name` on `{"status": null}`  
**Current behaviour:** `apply_tree(null, {name:{}})` hits the `other => other.clone()` arm → returns `null` → result is `{"status": null}`  
**Acceptable?** Yes. Null is valid JSON; the field exists but has no value. Consistent with how top-level nulls are handled.  
**Future fix:** none planned.

---

#### FIELDS-4 — Intermediate scalar on nested path returns scalar unchanged
**Found:** review session 2026-06-09  
**Trigger:** `--fields status.name` on `{"status": "open"}` (status is a string, not an object)  
**Current behaviour:** `apply_tree("open", {name:{}})` → returns `"open"` → result is `{"status": "open"}`. The `.name` segment is silently ignored.  
**Acceptable?** Marginal. The caller asked for `status.name` but gets the whole `status` value. Could be surprising if the API changes a field from scalar to object.  
**Future fix:** when the value at a non-leaf node is neither object nor array, either omit the key from the result or emit a stderr warning.

---

### auth.rs

#### AUTH-1 — Callback error param not surfaced clearly
**Found:** review session 2026-06-09  
**Trigger:** Atlassian redirects with `?error=access_denied&error_description=...` instead of `?code=...`  
**Current behaviour:** `parse_callback_request_line` returns `MissingParam("code")` — technically correct but the actual error reason (e.g. user denied consent) is in the `error` param which we never read.  
**Acceptable?** For now. The user sees "OAuth login failed: invalid OAuth callback: MissingParam("code")" — not great but rare path.  
**Future fix:** in `wait_for_callback`, after parsing params check for an `error` key and surface `error_description` as a dedicated `LoginError::ConsentDenied(String)` variant.

---

#### AUTH-2 — `OAuthConfig` does not validate non-empty client_id / client_secret
**Found:** review session 2026-06-09  
**Trigger:** `app.json` with `{"client_id": "", "client_secret": ""}` — parses successfully  
**Current behaviour:** empty strings pass `from_json`; the error surfaces later as a 401 from Atlassian with a generic message.  
**Acceptable?** Marginal. Early validation would give a clearer error.  
**Future fix:** add validation in `OAuthConfig::from_json` — return `InvalidJson` (or a new `EmptyCredential` variant) if either field is blank.
