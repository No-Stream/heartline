# TASK-126 findings for the rated-window scan

These observations use the task 1 isolated-piece harness at its held-speed, matching-slope and matching-bank approach. They do not change any rated window or determine where its new boundaries should be. The task 4 scan must reproduce and diagnose them before updating the catalog.

Briar Blitz's six-car `BankedStraightLeft` at 30 m/s passes in isolation: entry and minimum speed are both 30 m/s, with no force violation or clearance failure. `BankedStraightRight` also passes. The previous −2.4361 g lateral finding used an initially unfiltered train on a different, preceding-bank composition; it is not an isolated-piece rating.

A diagnostic scan of 1,178 existing endpoint/model/train combinations for level-to-level rated elements used `effective_rated_speeds` and construction-scaled train lengths. It found 28 non-passing combinations:

- Seven minimum-entry clearance failures: Loop for Timber Tumbler with 4 cars and Velocity Viper with 3 or 4 cars, and TopHat for Briar Blitz with 4–7 cars. The lowest observed speed was 4.28514 m/s for the 3-car Velocity loop against the existing 5 m/s clearance rule.
- Eighteen StallLeft and StallRight minimum-entry failures with calculated minima of 5.9999936–5.9999948 m/s against the existing 6 m/s clearance rule. Do not change the threshold to make these pass; determine whether the rated-bound calculation or numerical integration needs correction.
- Three Timber Tumbler eight-car ejector-hill maximum-entry vertical excursions: EjectorHill12x64, EjectorHill12x80 and EjectorHill16x96. At the first violation their filtered vertical forces were respectively −1.5804, −1.5115 and −1.5188 g; each was past the sustained limit for 0.205 s. The held-speed approach and excursion attribution need independent confirmation during the scan.

A separate held-speed scan evaluated filtered airtime at each current ejector maximum for allowed car counts. EjectorHill8x48 was classified Good rather than Great for the 4-car Timber train (−0.84568 g) and the 3- and 4-car Velocity trains (−0.73337 and −0.84568 g). WoodenEjectorHill6x48 was Good for Briar Blitz with 4–9 cars (−0.72349 to −0.88651 g). The existing raw-force airtime assertions remain in the test suite; the task 4 scan must investigate these filtered classifications against the Great-at-maximum acceptance criterion.
