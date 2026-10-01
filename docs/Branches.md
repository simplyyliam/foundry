1	app-shell	UI	— (merge this now)
2	backend/p0-skeleton	Backend	app-shell
3	ui/stage1-projects-document	UI	backend/p0-skeleton
4	backend/p1-projects	Backend	ui/stage1-projects-document (static half)
5	backend/p5-bakery	Backend	backend/p0-skeleton
6	backend/p7-sessions	Backend	backend/p0-skeleton
7	ui/stage2-blueprint-requirements	UI	backend/p1-projects
8	backend/p2-plan	Backend	ui/stage2-blueprint-requirements (static half)
9	ui/stage5-bakery-canvas	UI	backend/p5-bakery
10	ui/stage7-teams	UI	backend/p7-sessions
11	ui/stage3-roadmap-pane	UI	backend/p2-plan
12	backend/p3-roadmap	Backend	ui/stage3-roadmap-pane (static half)
13	ui/stage4-setup-team-composition	UI	backend/p3-roadmap
14	backend/p4-team-and-agents	Backend	ui/stage4-setup-team-composition (static half)
15	ui/stage6-prompts-view	UI	backend/p4-team-and-agents
16	backend/p6-prompts	Backend	ui/stage6-prompts-view (static half)
17	ui/stage8-agent-chats	UI	backend/p6-prompts
18	backend/p8-events	Backend	ui/stage8-agent-chats (static half)
19	ui/stage9-verification	UI	backend/p8-events
20	backend/p9-verification	Backend	ui/stage9-verification (static half)