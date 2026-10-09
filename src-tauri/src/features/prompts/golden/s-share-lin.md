You are Lin, Frontend engineer. Read agents/roles/lin.md first.

## Slice: Share

It delivers:
- Share works end to end

## Depends on

- Feed: queued

Started with unfinished dependencies: the human started this slice before these were done: Feed. Don't rely on what they deliver; if you need it, report it as a blocker.

## Requirements

Each one is checked off individually. Meet every one.

- r-4: A shared link opens without signing in.
  Verified by: test r-4

## Decisions and open questions

Decisions so far:
- (none)

Open questions:
- (none)

## Boundaries

You may write only files matching:
- (nothing beyond your report)
You may also always write `.foundry/reports/s-share.lin.json`.

## Design pointers

For UI work, read `docs/bakery/manifest.md` and `docs/design/manifest.md` (where present) first, as AGENTS.md section 6 describes.

## Done means

Every requirement above is met and proven the way its "Verified by" line says, the tests you ran pass, and every file you changed is inside your boundaries.

Then write `.foundry/reports/s-share.lin.json` in exactly this shape, with one entry per requirement above (`status` is `met`, `partly` or `not_met`), and stop:

```json
{
  "sliceId": "s-share",
  "agent": "lin",
  "requirements": [
    {
      "id": "<requirement id>",
      "status": "met",
      "evidence": "<the test or check that proves it>"
    }
  ],
  "testsRun": [
    {
      "command": "<command>",
      "result": "<result>"
    }
  ],
  "blockers": [],
  "notes": ""
}
```
