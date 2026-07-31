/**
 * A development-only run standing on a stand-in Field.
 *
 * **Not part of the game.** Nothing here is authored content, nothing here
 * ships: the module is reached by a dynamic import behind
 * `import.meta.env.DEV`, so a production build drops it along with the branch
 * that names it, and the production-build check reads the bundle to prove it.
 *
 * Why it exists: authored content arrives with a later goal, so a run today
 * stands on an empty Field — no Form, nothing to steer, and no way to see
 * steering work in a browser. The command set is closed and none of its nine
 * commands puts a Form on the Field, but one of them opens a run from a state
 * that already has one: `import_run` takes an export file, hash-verified and
 * validated like any other, and is valid in exactly the lifecycle state a fresh
 * worker is in. So the development run is an export file, and the core loads it
 * the same way it would load a run a player saved.
 *
 * The Field it carries is a small one: two layers, seven Nodes, three Routes,
 * two currents, and one controlled Form standing in the middle of the shallow
 * plane. It is a place to steer, not a chapter — no objective, no pressure, no
 * authored balance, and it is replaced outright by the goal that authors
 * content.
 *
 * Reach it by opening the local preview with the marker `field_run` in the
 * query string, which the worker client reads. The other marker,
 * `field_fixture`, does something different and cannot be combined with it: it
 * replaces the snapshots the renderer reads with a scripted stand-in, so the
 * core is not in the loop at all.
 *
 * How the bytes were made: a Field assembled from the same parts a chapter
 * declares was established on a run through `Run::establish_field`, and the
 * run's own `export_run` wrote this. Nothing was written by hand, and nothing
 * here can drift from the core's own reader without being refused by it: the
 * digest is over exactly these bytes, and the app test that opens this run
 * against a real worker fails the moment either stops agreeing.
 */

/** The export file the development run opens from. */
export const DEV_RUN_EXPORT =
  '{"format":"field-game-run","payload":{"anchors":[],"branch_nonce":0,"content_hash":"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","field":{"now":{"assembly_ordinal":0,"boundaries":{"authored":[],"drawn":[],"leak_frac":0},"currents":[{"active":true,"bright":true,"id":1,"layer":0,"path":[{"x":98304000,"y":137625600},{"x":117964800,"y":132382720},{"x":137625600,"y":131727360},{"x":157286400,"y":136314880},{"x":176947200,"y":145489920}],"period":30,"phase":0,"strength":1048576,"width":3670016},{"active":true,"bright":false,"id":2,"layer":1,"path":[{"x":117964800,"y":176947200},{"x":170393600,"y":163840000}],"period":45,"phase":0,"strength":1572864,"width":3145728}],"depth_cooldown":0,"forms":[{"charge":524288,"controlled":true,"focus":false,"forecast_depth":0,"form":"thread","id":1,"layer":0,"link":null,"node":1,"pos":{"x":134217728,"y":134217728},"pulse_charge":0,"reserve":0,"route_capacity":2097152,"route_reach":16777216,"steer_scale":65536,"trail":null,"vel":{"x":0,"y":0}}],"layers":[{"current_ids":[1],"drain":0,"gain":65536,"layer":0,"noise":0,"port_ids":[2,3,4,5]},{"current_ids":[2],"drain":65536,"gain":65536,"layer":1,"noise":0,"port_ids":[6,7]}],"next_node_id":8,"next_route_id":4,"pending":[],"ports":[{"capacity":33554432,"kind":"form","layer":0,"node":1,"open":true,"pos":{"x":134217728,"y":134217728},"q":524288,"upkeep_rate":0},{"capacity":33554432,"kind":"port","layer":0,"node":2,"open":false,"pos":{"x":115343360,"y":123207680},"q":1572864,"upkeep_rate":0},{"capacity":33554432,"kind":"reserve","layer":0,"node":3,"open":true,"pos":{"x":154664960,"y":124518400},"q":6291456,"upkeep_rate":0},{"capacity":33554432,"kind":"module","layer":0,"node":4,"open":true,"pos":{"x":150732800,"y":153354240},"q":2621440,"upkeep_rate":0},{"capacity":33554432,"kind":"port","layer":0,"node":5,"open":false,"pos":{"x":112721920,"y":152043520},"q":786432,"upkeep_rate":0},{"capacity":33554432,"kind":"reserve","layer":1,"node":6,"open":true,"pos":{"x":129761280,"y":170393600},"q":4194304,"upkeep_rate":0},{"capacity":33554432,"kind":"module","layer":1,"node":7,"open":true,"pos":{"x":163840000,"y":173670400},"q":2097152,"upkeep_rate":0}],"prev_assembly_step":null,"routes":[{"capacity":1048576,"flow":0,"formed_step":0,"head":3,"route":1,"tail":2},{"capacity":786432,"flow":0,"formed_step":0,"head":4,"route":2,"tail":3},{"capacity":524288,"flow":0,"formed_step":0,"head":7,"route":3,"tail":6}],"step":0,"wheel_accum":0},"trace":{"keyframe":{"assembly_ordinal":0,"boundaries":{"authored":[],"drawn":[],"leak_frac":0},"currents":[{"active":true,"bright":true,"id":1,"layer":0,"path":[{"x":98304000,"y":137625600},{"x":117964800,"y":132382720},{"x":137625600,"y":131727360},{"x":157286400,"y":136314880},{"x":176947200,"y":145489920}],"period":30,"phase":0,"strength":1048576,"width":3670016},{"active":true,"bright":false,"id":2,"layer":1,"path":[{"x":117964800,"y":176947200},{"x":170393600,"y":163840000}],"period":45,"phase":0,"strength":1572864,"width":3145728}],"depth_cooldown":0,"forms":[{"charge":524288,"controlled":true,"focus":false,"forecast_depth":0,"form":"thread","id":1,"layer":0,"link":null,"node":1,"pos":{"x":134217728,"y":134217728},"pulse_charge":0,"reserve":0,"route_capacity":2097152,"route_reach":16777216,"steer_scale":65536,"trail":null,"vel":{"x":0,"y":0}}],"layers":[{"current_ids":[1],"drain":0,"gain":65536,"layer":0,"noise":0,"port_ids":[2,3,4,5]},{"current_ids":[2],"drain":65536,"gain":65536,"layer":1,"noise":0,"port_ids":[6,7]}],"next_node_id":8,"next_route_id":4,"pending":[],"ports":[{"capacity":33554432,"kind":"form","layer":0,"node":1,"open":true,"pos":{"x":134217728,"y":134217728},"q":524288,"upkeep_rate":0},{"capacity":33554432,"kind":"port","layer":0,"node":2,"open":false,"pos":{"x":115343360,"y":123207680},"q":1572864,"upkeep_rate":0},{"capacity":33554432,"kind":"reserve","layer":0,"node":3,"open":true,"pos":{"x":154664960,"y":124518400},"q":6291456,"upkeep_rate":0},{"capacity":33554432,"kind":"module","layer":0,"node":4,"open":true,"pos":{"x":150732800,"y":153354240},"q":2621440,"upkeep_rate":0},{"capacity":33554432,"kind":"port","layer":0,"node":5,"open":false,"pos":{"x":112721920,"y":152043520},"q":786432,"upkeep_rate":0},{"capacity":33554432,"kind":"reserve","layer":1,"node":6,"open":true,"pos":{"x":129761280,"y":170393600},"q":4194304,"upkeep_rate":0},{"capacity":33554432,"kind":"module","layer":1,"node":7,"open":true,"pos":{"x":163840000,"y":173670400},"q":2097152,"upkeep_rate":0}],"prev_assembly_step":null,"routes":[{"capacity":1048576,"flow":0,"formed_step":0,"head":3,"route":1,"tail":2},{"capacity":786432,"flow":0,"formed_step":0,"head":4,"route":2,"tail":3},{"capacity":524288,"flow":0,"formed_step":0,"head":7,"route":3,"tail":6}],"step":0,"wheel_accum":0},"start_step":0,"steps":[]}},"input_config":{"bindings":{"ascend":"BracketLeft","cancel":"Escape","commit":"Enter","descend":"BracketRight","down":"KeyS","left":"KeyA","pulse":"ShiftLeft","right":"KeyD","still":"Space","up":"KeyW"},"pointer_speed":65536,"reduced_motion":false,"sound_level":65536,"trail_intensity":65536},"pressures":[],"progress":{"chapter_index":0,"complete":[],"impulse":3,"objective":{"completed_step":null,"id":"","progress":0,"started_step":0,"state":"hidden","target":null}},"rng":{"ctr":"00000000000000000000000000000000","half":0,"key":"fde2381b87ec0e4a"},"run_id":"0123456789abcdef","save_version":1,"slate":null,"view":{"inside":[2,3,4],"resolution":1,"surround":"adjacent","window":45}},"payload_sha256":"891785c17e9ad6425106e06172210fa310077a7a652208d2959c9b705ff0693e","save_version":1}';
