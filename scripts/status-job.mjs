#!/usr/bin/env node
// Lists a pacemaker's jobs. Read-only: needs no key, no account and no wallet.
//
//   node scripts/status-job.mjs <pacemaker>
import { argv, view } from "./lib.mjs";

const pacemaker = argv().positional[0];
if (!pacemaker) {
  console.error("usage: node scripts/status-job.mjs <pacemaker>");
  process.exit(1);
}
const jobs = await view(pacemaker, "jobs", {});
if (!jobs.length) console.log("No jobs.");
for (const [id, job] of jobs) {
  console.log(`job ${id}: ${job.target}.${job.method}() every ${job.every_beats} beat(s), ${job.beats} beats, ${job.calls} calls, last at block ${job.last_beat_block}, owner ${job.owner}`);
}
