# Crest turn point-mass feasibility

Each cell shows whether the auto-height offline point-mass solver accepts that bank and apex feel.
It checks 0.25 g headroom against the hyper's -1.0 to 4.0 g vertical limits, the 0.15 g fully banked lateral limit,
the 200 Hz ASTM comfort evaluator, roll rate, and both horizontal coordinates on the 2 m grid for all eight entry headings.
Longitudinal force is zero in the lossless energy model. This map does not include train cars, drag, clearance, or supports.

Fixed-height mode adds apex height as a constraint and solves apex-hold length as a third unknown. Grid placement re-solves that schedule and accepts the rounded caller target within 1e-6 m; a discrete snap that cannot meet this tolerance returns a typed refusal. The 12 m target at 26 m/s, 90-degree sweep, 90-degree bank, floaty feel and East entry is covered as a successful 2 m-step placement. The stored ramp, pull hold, ease, apex hold, heading, and scale let accepted geometry replay its placement. Inputs and resolved schedule values are rounded to nine decimals.

A refusal means this fixed deterministic recipe missed its numerical or listed constraints; it does not prove that no other schedule can work.

| Sweep | Apex bank | Apex feel | 26 m/s | 30 m/s | 34 m/s | 38 m/s | 42 m/s |
| --- | ---: | --- | ---: | ---: | ---: | ---: | ---: |
| 90° | 90° | floaty (0.3 g) | solve | solve | solve | refuse | refuse |
| 90° | 95° | floaty (0.3 g) | solve | solve | solve | solve | solve |
| 90° | 100° | floaty (0.3 g) | solve | solve | solve | solve | solve |
| 90° | 105° | floaty (0.3 g) | solve | solve | solve | solve | solve |
| 90° | 110° | floaty (0.3 g) | solve | solve | solve | solve | solve |
| 90° | 115° | floaty (0.3 g) | refuse | solve | solve | solve | solve |
| 90° | 120° | floaty (0.3 g) | refuse | solve | solve | solve | solve |
| 90° | 90° | neutral (1.0 g) | refuse | refuse | refuse | refuse | refuse |
| 90° | 95° | neutral (1.0 g) | solve | solve | solve | refuse | refuse |
| 90° | 100° | neutral (1.0 g) | refuse | solve | solve | solve | solve |
| 90° | 105° | neutral (1.0 g) | refuse | solve | solve | solve | solve |
| 90° | 110° | neutral (1.0 g) | refuse | refuse | solve | solve | solve |
| 90° | 115° | neutral (1.0 g) | refuse | refuse | solve | solve | solve |
| 90° | 120° | neutral (1.0 g) | refuse | refuse | solve | solve | solve |
| 90° | 90° | pressed-in (1.5 g) | refuse | refuse | refuse | refuse | refuse |
| 90° | 95° | pressed-in (1.5 g) | refuse | solve | refuse | refuse | refuse |
| 90° | 100° | pressed-in (1.5 g) | refuse | solve | solve | solve | solve |
| 90° | 105° | pressed-in (1.5 g) | refuse | refuse | solve | solve | solve |
| 90° | 110° | pressed-in (1.5 g) | refuse | refuse | solve | solve | solve |
| 90° | 115° | pressed-in (1.5 g) | refuse | refuse | refuse | solve | solve |
| 90° | 120° | pressed-in (1.5 g) | refuse | refuse | refuse | solve | solve |
| 180° | 90° | floaty (0.3 g) | solve | solve | solve | solve | solve |
| 180° | 95° | floaty (0.3 g) | solve | solve | solve | solve | solve |
| 180° | 100° | floaty (0.3 g) | refuse | solve | solve | solve | solve |
| 180° | 105° | floaty (0.3 g) | solve | solve | solve | solve | solve |
| 180° | 110° | floaty (0.3 g) | solve | solve | solve | solve | solve |
| 180° | 115° | floaty (0.3 g) | solve | solve | solve | solve | solve |
| 180° | 120° | floaty (0.3 g) | refuse | refuse | solve | solve | solve |
| 180° | 90° | neutral (1.0 g) | solve | solve | solve | solve | solve |
| 180° | 95° | neutral (1.0 g) | solve | solve | solve | solve | solve |
| 180° | 100° | neutral (1.0 g) | solve | solve | solve | solve | solve |
| 180° | 105° | neutral (1.0 g) | solve | solve | solve | solve | solve |
| 180° | 110° | neutral (1.0 g) | solve | solve | solve | solve | solve |
| 180° | 115° | neutral (1.0 g) | refuse | solve | solve | solve | solve |
| 180° | 120° | neutral (1.0 g) | refuse | solve | solve | solve | solve |
| 180° | 90° | pressed-in (1.5 g) | solve | refuse | refuse | refuse | refuse |
| 180° | 95° | pressed-in (1.5 g) | solve | solve | solve | solve | solve |
| 180° | 100° | pressed-in (1.5 g) | solve | solve | solve | solve | solve |
| 180° | 105° | pressed-in (1.5 g) | solve | solve | solve | solve | solve |
| 180° | 110° | pressed-in (1.5 g) | refuse | solve | solve | solve | solve |
| 180° | 115° | pressed-in (1.5 g) | refuse | solve | solve | solve | solve |
| 180° | 120° | pressed-in (1.5 g) | refuse | refuse | solve | solve | solve |
