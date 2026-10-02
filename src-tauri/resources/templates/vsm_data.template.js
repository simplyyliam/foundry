// vsm_data.js: the value stream map that vsm_viewer.html renders. Rename this file to
// vsm_data.js in the project root and fill it in. (In a Foundry project it's generated from
// .foundry/plan.json instead: don't hand-edit it there, the next plan change overwrites it.)
//
// Schema (read by vsm_viewer.html):
//   meta: { title, subtitle }
//   slices: [{
//     id, name,
//     order: 1 for slices with no dependencies, otherwise 1 + the longest chain below it;
//            slices with the same order can be built in parallel,
//     status: "queued" | "in_progress" | "done",
//     depends_on: [slice ids], delivers: [capabilities],
//     used_by: how many slices depend on this one,
//     resolved: [decisions made], open: [open questions]
//   }]
//   future_horizon: [{ id, label, anchored_to: slice id }]: deferred features. Don't build
//     interfaces that block them (AGENTS.md, Law 8).

const VSM_DATA = {
  "meta": {
    "title": "",
    "subtitle": ""
  },
  "slices": [],
  "future_horizon": []
};
