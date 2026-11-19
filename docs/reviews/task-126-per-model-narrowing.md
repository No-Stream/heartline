# TASK-126: diagnosis of the partial scan

The 42 decreases greater than 1.5 m/s in `window-changes-partial.md` comprise 24 banked-straight and 18 rollout model/piece/slope windows. All 42 boundaries are lateral-force violations immediately after a bank-transition approach, in the connected measurement. They are real limits for the particular buildable approach used by the scan; they are not proven universal limits for the piece under every possible preceding track. The partial scan did not write a generated table, and these values must not be treated as a complete or accepted table.

## Cause and competing explanations

* **Connected bank transition (42):** each approach contains a `BankLeft` or `BankRight` transition. The sloped variants first enter the corresponding gentle slope. Construction offers both the banked straight and rollout after that transition, and the catalog offers sloped transitions. The model-valid route fix described below does not change any of these paths. The filtered forces from the approach continue across the joint, and every first failing cent is lateral, at or close to the start of the rated piece. The detailed measurements compare connected and isolated tracks at both endpoints in `force-diagnostics-42.log`.
* **Old design cap (24 banked straights):** the old maximum was 30 m/s and marked `DesignCap`, but this does **not** mean the old scanner skipped the force check. At commit `862427b`, it called `failure` on the old maximum before preserving the cap. That check used the isolated track, whose fixed-bank force differs from the connected transition. The connected limit is newly exposed; it is not established merely by the design-cap label.
* **Old force bound (18 rollouts):** the old maxima were 45.87, 52.16, or 55.33 m/s depending on slope. Their new boundaries nearly match the corresponding banked straight because the same preceding bank transition dominates. These are not previously unmeasured design caps.
* **Model and train selection:** the model definitions are unchanged since `862427b`. The old scan checked every allowed model and every allowed car count, including each current default. Selecting a default alone cannot narrow a window on the same track. The new scan passes the selected model's own train and comfortable limits to the evaluator. No wrong-model, wrong-length, or mixed-slope bug was established. Model limits do explain differences in the connected boundaries: maximum lateral sustained limits are 1.0 g for family, 1.2 g for hyper, and 1.5 g for the other three models; the duration evaluator additionally allows up to 0.5 g above sustained for at most 0.2 s.

## Boundary evidence

Each row is one of the 42 model/piece/slope windows. Speeds are in m/s. Both force columns show lateral g, zero-based car index, and excursion duration in seconds above that model's sustained lateral limit. At the passing maximum, the row reports the largest absolute filtered lateral force during the rated piece and tail and the duration of that car's associated excursion, including its time on the approach. The first rejected cent reports the evaluator's first violation and the duration and peak of that excursion. A value beyond the brief cap can fail before 0.2 s; a lower peak can fail when an excursion lasts longer. The full log includes distance and isolated-track comparisons.

| Model | Piece | Slope | Old max | Passing max | At passing max: g / car / s | First rejected | At rejected speed: g / car / s |
| --- | --- | --- | ---: | ---: | --- | ---: | --- |
| family_coaster | BankedStraightLeft | DownGentle | 30.00 | 17.20 | -1.244 / 9 / 0.200 | 17.21 | -1.010 / 9 / 0.205 |
| family_coaster | BankedStraightLeft | Level | 30.00 | 14.81 | -0.998 / 7 / 0.000 | 14.82 | -1.001 / 4 / 0.250 |
| family_coaster | BankedStraightRight | DownGentle | 30.00 | 17.20 | +1.244 / 9 / 0.200 | 17.21 | +1.010 / 9 / 0.205 |
| family_coaster | BankedStraightRight | Level | 30.00 | 14.81 | +0.998 / 7 / 0.000 | 14.82 | +1.001 / 4 / 0.250 |
| hybrid | BankedStraightLeft | DownGentle | 30.00 | 26.59 | -1.991 / 5 / 0.135 | 26.60 | -2.001 / 5 / 0.135 |
| hybrid | BankedStraightLeft | Level | 30.00 | 25.48 | -1.914 / 5 / 0.180 | 25.49 | -1.535 / 2 / 0.205 |
| hybrid | BankedStraightRight | DownGentle | 30.00 | 26.59 | +1.991 / 5 / 0.135 | 26.60 | +2.001 / 5 / 0.135 |
| hybrid | BankedStraightRight | Level | 30.00 | 25.48 | +1.914 / 5 / 0.180 | 25.49 | +1.535 / 2 / 0.205 |
| hybrid | RolloutLeft | DownGentle | 52.16 | 26.58 | -1.991 / 5 / 0.135 | 26.59 | -2.001 / 5 / 0.135 |
| hybrid | RolloutLeft | Level | 45.87 | 25.48 | -1.915 / 5 / 0.180 | 25.49 | -1.535 / 2 / 0.205 |
| hybrid | RolloutLeft | UpGentle | 55.33 | 28.80 | -1.994 / 1 / 0.155 | 28.81 | -2.020 / 1 / 0.155 |
| hybrid | RolloutRight | DownGentle | 52.16 | 26.58 | +1.991 / 5 / 0.135 | 26.59 | +2.001 / 5 / 0.135 |
| hybrid | RolloutRight | Level | 45.87 | 25.48 | +1.915 / 5 / 0.180 | 25.49 | +1.535 / 2 / 0.205 |
| hybrid | RolloutRight | UpGentle | 55.33 | 28.80 | +1.994 / 1 / 0.155 | 28.81 | +2.020 / 1 / 0.155 |
| hyper | BankedStraightLeft | DownGentle | 30.00 | 22.72 | -1.696 / 7 / 0.180 | 22.73 | -1.711 / 7 / 0.175 |
| hyper | BankedStraightLeft | Level | 30.00 | 18.12 | -1.213 / 5 / 0.195 | 18.13 | -1.200 / 2 / 0.280 |
| hyper | BankedStraightLeft | UpGentle | 30.00 | 22.61 | -1.427 / 1 / 0.150 | 22.62 | -1.218 / 2 / 0.205 |
| hyper | BankedStraightRight | DownGentle | 30.00 | 22.72 | +1.696 / 7 / 0.180 | 22.73 | +1.711 / 7 / 0.175 |
| hyper | BankedStraightRight | Level | 30.00 | 18.12 | +1.213 / 5 / 0.195 | 18.13 | +1.200 / 2 / 0.280 |
| hyper | BankedStraightRight | UpGentle | 30.00 | 22.61 | +1.427 / 1 / 0.150 | 22.62 | +1.218 / 2 / 0.205 |
| velocity_viper | BankedStraightLeft | DownGentle | 30.00 | 26.59 | -1.991 / 5 / 0.135 | 26.60 | -2.001 / 5 / 0.135 |
| velocity_viper | BankedStraightLeft | Level | 30.00 | 25.48 | -1.914 / 5 / 0.180 | 25.49 | -1.535 / 2 / 0.205 |
| velocity_viper | BankedStraightRight | DownGentle | 30.00 | 26.59 | +1.991 / 5 / 0.135 | 26.60 | +2.001 / 5 / 0.135 |
| velocity_viper | BankedStraightRight | Level | 30.00 | 25.48 | +1.914 / 5 / 0.180 | 25.49 | +1.535 / 2 / 0.205 |
| velocity_viper | RolloutLeft | DownGentle | 52.16 | 26.58 | -1.991 / 5 / 0.135 | 26.59 | -2.001 / 5 / 0.135 |
| velocity_viper | RolloutLeft | Level | 45.87 | 25.48 | -1.915 / 5 / 0.180 | 25.49 | -1.535 / 2 / 0.205 |
| velocity_viper | RolloutLeft | UpGentle | 55.33 | 28.80 | -1.994 / 1 / 0.155 | 28.81 | -2.020 / 1 / 0.155 |
| velocity_viper | RolloutRight | DownGentle | 52.16 | 26.58 | +1.991 / 5 / 0.135 | 26.59 | +2.001 / 5 / 0.135 |
| velocity_viper | RolloutRight | Level | 45.87 | 25.48 | +1.915 / 5 / 0.180 | 25.49 | +1.535 / 2 / 0.205 |
| velocity_viper | RolloutRight | UpGentle | 55.33 | 28.80 | +1.994 / 1 / 0.155 | 28.81 | +2.020 / 1 / 0.155 |
| wooden_twister | BankedStraightLeft | DownGentle | 30.00 | 26.27 | -1.991 / 11 / 0.140 | 26.28 | -1.574 / 3 / 0.210 |
| wooden_twister | BankedStraightLeft | Level | 30.00 | 21.51 | -1.501 / 4 / 0.130 | 21.52 | -1.500 / 3 / 0.210 |
| wooden_twister | BankedStraightLeft | UpGentle | 30.00 | 25.59 | -1.691 / 2 / 0.145 | 25.60 | -1.530 / 3 / 0.205 |
| wooden_twister | BankedStraightRight | DownGentle | 30.00 | 26.27 | +1.991 / 11 / 0.140 | 26.28 | +1.574 / 3 / 0.210 |
| wooden_twister | BankedStraightRight | Level | 30.00 | 21.51 | +1.501 / 4 / 0.130 | 21.52 | +1.500 / 3 / 0.210 |
| wooden_twister | BankedStraightRight | UpGentle | 30.00 | 25.59 | +1.691 / 2 / 0.145 | 25.60 | +1.530 / 3 / 0.205 |
| wooden_twister | RolloutLeft | DownGentle | 52.16 | 26.27 | -1.992 / 11 / 0.140 | 26.28 | -1.574 / 3 / 0.210 |
| wooden_twister | RolloutLeft | Level | 45.87 | 21.51 | -1.501 / 4 / 0.130 | 21.52 | -1.500 / 3 / 0.210 |
| wooden_twister | RolloutLeft | UpGentle | 55.33 | 25.59 | -1.691 / 2 / 0.145 | 25.60 | -1.530 / 3 / 0.205 |
| wooden_twister | RolloutRight | DownGentle | 52.16 | 26.27 | +1.992 / 11 / 0.140 | 26.28 | +1.574 / 3 / 0.210 |
| wooden_twister | RolloutRight | Level | 45.87 | 21.51 | +1.501 / 4 / 0.130 | 21.52 | +1.500 / 3 / 0.210 |
| wooden_twister | RolloutRight | UpGentle | 55.33 | 25.59 | +1.691 / 2 / 0.145 | 25.60 | +1.530 / 3 / 0.205 |

## Banked-straight generation failure

Both failures in the log are **family_coaster, BankedStraightLeft/Right, UpGentle to UpGentle**, with a 10-car default train. The catalog minimum is **20 m/s**, not 5 m/s. At 20 m/s the connected trace fails on car 3: lateral −1.200220 g on the left (+1.200220 g on the right), for 0.270 s above the 1.0 g sustained limit, with the reported excursion peak 0.079 m after entry. The isolated trace passes with lateral −/+0.632456 g. The scan looked above the minimum and found no passing grid speed up to its ceiling; the old panic omitted the model and slope. See `banked-straight-failure.log`.

A fixed 45° bank can produce lateral gravity force at any speed. On level track its magnitude is sin(45°) = 0.707 g, and on this gentle slope it is 0.632 g. Both are below this model's 1.0 g sustained lateral limit, so a lateral-force failure at 5 m/s would not follow from the fixed bank alone for this model. The actual uphill 5 m/s probe stalls or rolls back; that speed is below its catalog minimum. All five models pass the level and downhill banked-straight force evaluation at 5 m/s in the focused checks. The 20 m/s failure is the connected bank-transition excursion, not a sustained fixed-bank gravity violation.

## Confirmed approach bug and scope

The old connected-track builder hardcoded approach prefixes without checking whether the model supported their pieces. The regression added in `rated_element_physics.rs` failed on Horizon Hawk's `SixtyDownToLevel` approach before the fix. The builder now searches connector states using only pieces allowed by the selected model, and the regression passes across rated catalog pieces and eligible models. This fixes invalid approaches outside the 42 cases, and the route comparison found no change to any banked-straight or rollout approach in this report.

The fix also changes the chosen route in 15 model/target combinations covered by the historical hardcoded prefixes, including valid routes for Timber Tumbler and Velocity Viper. This is a material measurement-context change: scan results for other pieces may change and must be reviewed, rather than attributed solely to the model key. A single connected route does not establish behavior for every legal predecessor. The per-model table must be regenerated and reviewed only after deciding how its context-dependent limits should be represented; no table was generated in this diagnosis. No force limits were weakened.

## Verification and remaining work

The new model-allowed-approach regression passed after it first failed on the unsupported Horizon Hawk prefix. The existing rated-window endpoint test cannot complete against the current temporary generated table: its model keys are empty, and it reports `hybrid LevelToSixtyUp has no measured rated window`. The workspace formatting check also found differences, including files outside this diagnosis. These gates need a clean rerun once the incomplete table and formatting are resolved. The scan's no-passing-window case needs an explicit product decision on how to represent an unavailable model/piece/slope combination before generation can succeed; silently raising or weakening a limit would hide the result.

Checks in this diagnosis: `make test-rated-windows` passed 6/6; `tools/run-in-budget.sh cargo test -p theme-park-sim --lib physics::rated_piece::tests` passed 11/11; the model-allowed-approach regression passed 1/1; and `git diff --check` passed. `tools/run-in-budget.sh cargo fmt --all -- --check` failed on formatting differences in other files, including `crates/sim/examples/circuit_report.rs` and `crates/sim/src/physics/block_operation.rs`; the three files changed for this diagnosis were formatted. Resolving the formatting differences and rerunning the workspace check is required before a clean gate. The broader table-dependent tests require a usable generated table, which this task explicitly prohibits generating.
