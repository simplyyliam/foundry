You are Ada, Backend engineer. Read agents/roles/ada.md first.

## Slice: Feed

It delivers:
- Feed works end to end

## Depends on

- Sign in: done

## Requirements

Each one is checked off individually. Meet every one.

- r-2: The feed shows the newest highlight first.
  Verified by: test r-2
- r-3: Pressing J moves to the next highlight; K moves back.
  Verified by: test r-3

## Decisions and open questions

Decisions so far:
- Use cursor paging

Open questions:
- Infinite scroll or pages?

## Boundaries

You may write only files matching:
- `src/server/**`
- `src/features/feed/**`
You may also always write `.foundry/reports/s-feed.json`.

## Design pointers

For UI work, read `docs/bakery/manifest.md` and `docs/design/manifest.md` (where present) first, as AGENTS.md section 6 describes.

## Done means

Every requirement above is met and proven the way its "Verified by" line says, the tests you ran pass, and every file you changed is inside your boundaries.

Then write `.foundry/reports/s-feed.json` in exactly this shape, with one entry per requirement above (`status` is `met`, `partly` or `not_met`), and stop:

```json
{
  "sliceId": "s-feed",
  "agent": "ada",
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
