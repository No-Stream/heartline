# TASK-126 approach-context window changes

Compared with commit 5699631. Speeds are in m/s. Reachability minima arise when the fixed unpowered pre-roll cannot reach lower entry speeds; they are not force violations.

```text
Old measured rows: 351; new measured rows: 349
Changed windows by model (speeds in m/s):
  family_coaster BankLeft (DownGentle->DownGentle): 5.000–12.370 -> 5.410–11.680 (min +0.410; max -0.690; sources EnergyMinimum/ForceBound -> ApproachMinimum/ForceBound)
  family_coaster BankLeft (UpGentle->UpGentle): 17.010–20.760 -> 17.330–20.750 (min +0.320; max -0.010; sources ClearanceMinimum/ForceBound -> ClearanceMinimum/ForceBound)
  family_coaster BankRight (DownGentle->DownGentle): 5.000–12.370 -> 5.410–11.680 (min +0.410; max -0.690; sources EnergyMinimum/ForceBound -> ApproachMinimum/ForceBound)
  family_coaster BankRight (UpGentle->UpGentle): 17.010–20.760 -> 17.330–20.750 (min +0.320; max -0.010; sources ClearanceMinimum/ForceBound -> ClearanceMinimum/ForceBound)
  family_coaster BankedStraightLeft (DownGentle->DownGentle): 5.000–30.000 -> 9.350–17.160 (min +4.350; max -12.840; sources EnergyMinimum/DesignCap -> ApproachMinimum/ForceBound)
  family_coaster BankedStraightLeft (Level->Level): 5.000–30.000 -> 5.000–14.810 (min +0.000; max -15.190; sources EnergyMinimum/DesignCap -> EnergyMinimum/ForceBound)
  UNAVAILABLE ('family_coaster', 'BankedStraightLeft', 'UpGentle', 'UpGentle'): old 20.000–30.000 -> new unavailable
  family_coaster BankedStraightRight (DownGentle->DownGentle): 5.000–30.000 -> 9.350–17.160 (min +4.350; max -12.840; sources EnergyMinimum/DesignCap -> ApproachMinimum/ForceBound)
  family_coaster BankedStraightRight (Level->Level): 5.000–30.000 -> 5.000–14.810 (min +0.000; max -15.190; sources EnergyMinimum/DesignCap -> EnergyMinimum/ForceBound)
  UNAVAILABLE ('family_coaster', 'BankedStraightRight', 'UpGentle', 'UpGentle'): old 20.000–30.000 -> new unavailable
  family_coaster UnbankLeft (DownGentle->DownGentle): 5.000–13.710 -> 9.370–13.910 (min +4.370; max +0.200; sources EnergyMinimum/ForceBound -> ApproachMinimum/ForceBound)
  family_coaster UnbankLeft (Level->Level): 5.000–13.400 -> 5.000–13.390 (min +0.000; max -0.010; sources EnergyMinimum/ForceBound -> EnergyMinimum/ForceBound)
  family_coaster UnbankLeft (UpGentle->UpGentle): 17.330–21.690 -> 17.330–22.310 (min +0.000; max +0.620; sources ClearanceMinimum/ForceBound -> ClearanceMinimum/ForceBound)
  family_coaster UnbankRight (DownGentle->DownGentle): 5.000–13.710 -> 9.370–13.910 (min +4.370; max +0.200; sources EnergyMinimum/ForceBound -> ApproachMinimum/ForceBound)
  family_coaster UnbankRight (Level->Level): 5.000–13.400 -> 5.000–13.390 (min +0.000; max -0.010; sources EnergyMinimum/ForceBound -> EnergyMinimum/ForceBound)
  family_coaster UnbankRight (UpGentle->UpGentle): 17.330–21.690 -> 17.330–22.310 (min +0.000; max +0.620; sources ClearanceMinimum/ForceBound -> ClearanceMinimum/ForceBound)
  hybrid BankLeft (DownGentle->DownGentle): 5.000–23.760 -> 5.250–23.430 (min +0.250; max -0.330; sources EnergyMinimum/ForceBound -> ApproachMinimum/ForceBound)
  hybrid BankLeft (UpGentle->UpGentle): 17.240–30.180 -> 17.750–30.190 (min +0.510; max +0.010; sources ClearanceMinimum/ForceBound -> ClearanceMinimum/ForceBound)
  hybrid BankRight (DownGentle->DownGentle): 5.000–23.760 -> 5.250–23.430 (min +0.250; max -0.330; sources EnergyMinimum/ForceBound -> ApproachMinimum/ForceBound)
  hybrid BankRight (UpGentle->UpGentle): 17.240–30.180 -> 17.750–30.190 (min +0.510; max +0.010; sources ClearanceMinimum/ForceBound -> ClearanceMinimum/ForceBound)
  hybrid BankedStraightLeft (DownGentle->DownGentle): 5.000–30.000 -> 9.320–26.690 (min +4.320; max -3.310; sources EnergyMinimum/DesignCap -> ApproachMinimum/ForceBound)
  hybrid BankedStraightLeft (Level->Level): 5.000–30.000 -> 5.000–26.120 (min +0.000; max -3.880; sources EnergyMinimum/DesignCap -> EnergyMinimum/ForceBound)
  hybrid BankedStraightLeft (UpGentle->UpGentle): 20.000–30.000 -> 20.000–28.000 (min +0.000; max -2.000; sources EnergyMinimum/DesignCap -> EnergyMinimum/ForceBound)
  hybrid BankedStraightRight (DownGentle->DownGentle): 5.000–30.000 -> 9.320–26.690 (min +4.320; max -3.310; sources EnergyMinimum/DesignCap -> ApproachMinimum/ForceBound)
  hybrid BankedStraightRight (Level->Level): 5.000–30.000 -> 5.000–26.120 (min +0.000; max -3.880; sources EnergyMinimum/DesignCap -> EnergyMinimum/ForceBound)
  hybrid BankedStraightRight (UpGentle->UpGentle): 20.000–30.000 -> 20.000–28.000 (min +0.000; max -2.000; sources EnergyMinimum/DesignCap -> EnergyMinimum/ForceBound)
  hybrid EjectorHill8x48 (Level->Level): 13.199–19.650 -> 13.199–19.660 (min +0.000; max +0.010; sources EnergyMinimum/AirtimeFeel -> EnergyMinimum/AirtimeFeel)
  hybrid FamilyHill4x40 (Level->Level): 9.967–17.280 -> 9.967–17.290 (min +0.000; max +0.010; sources EnergyMinimum/AirtimeFeel -> EnergyMinimum/AirtimeFeel)
  hybrid HalfCorkscrewLeftDown (Level->Level): 5.000–15.766 -> 5.000–15.760 (min +0.000; max -0.006; sources EnergyMinimum/DesignCap -> EnergyMinimum/ClearanceMaximum)
  hybrid HalfCorkscrewRightDown (Level->Level): 5.000–15.766 -> 5.000–15.760 (min +0.000; max -0.006; sources EnergyMinimum/DesignCap -> EnergyMinimum/ClearanceMaximum)
  hybrid HalfLoopDown (Level->Level): 4.000–20.470 -> 4.000–20.100 (min +0.000; max -0.370; sources EnergyMinimum/ForceBound -> EnergyMinimum/ForceBound)
  hybrid RavenRollLeftDown (Level->Level): 8.000–20.210 -> 8.000–19.840 (min +0.000; max -0.370; sources EnergyMinimum/ForceBound -> EnergyMinimum/ForceBound)
  hybrid RavenRollLeftUp (Level->Level): 8.000–18.320 -> 8.000–18.330 (min +0.000; max +0.010; sources EnergyMinimum/ForceBound -> EnergyMinimum/ForceBound)
  hybrid RavenRollRightDown (Level->Level): 8.000–20.210 -> 8.000–19.840 (min +0.000; max -0.370; sources EnergyMinimum/ForceBound -> EnergyMinimum/ForceBound)
  hybrid RavenRollRightUp (Level->Level): 8.000–18.320 -> 8.000–18.330 (min +0.000; max +0.010; sources EnergyMinimum/ForceBound -> EnergyMinimum/ForceBound)
  hybrid RolloutLeft (DownGentle->DownGentle): 5.000–52.660 -> 9.370–52.670 (min +4.370; max +0.010; sources EnergyMinimum/ForceBound -> ApproachMinimum/ForceBound)
  hybrid RolloutLeft (UpGentle->UpGentle): 22.000–55.340 -> 22.000–55.290 (min +0.000; max -0.050; sources EnergyMinimum/ForceBound -> EnergyMinimum/ForceBound)
  hybrid RolloutRight (DownGentle->DownGentle): 5.000–52.660 -> 9.370–52.670 (min +4.370; max +0.010; sources EnergyMinimum/ForceBound -> ApproachMinimum/ForceBound)
  hybrid RolloutRight (UpGentle->UpGentle): 22.000–55.340 -> 22.000–55.290 (min +0.000; max -0.050; sources EnergyMinimum/ForceBound -> EnergyMinimum/ForceBound)
  hybrid SixtyDownToLevel (DownSixty->Level): 2.000–25.190 -> 12.560–25.160 (min +10.560; max -0.030; sources EnergyMinimum/ForceBound -> ApproachMinimum/ForceBound)
  hybrid SixtyToSteepDown (DownSixty->DownSteep): 2.000–30.910 -> 12.560–30.890 (min +10.560; max -0.020; sources EnergyMinimum/ForceBound -> ApproachMinimum/ForceBound)
  hybrid SixtyToSteepUp (UpSixty->UpSteep): 27.000–38.100 -> 27.000–37.960 (min +0.000; max -0.140; sources EnergyMinimum/ForceBound -> EnergyMinimum/ForceBound)
  hybrid SteepToSixtyDown (DownSteep->DownSixty): 3.000–27.920 -> 10.510–27.940 (min +7.510; max +0.020; sources EnergyMinimum/ForceBound -> ApproachMinimum/ForceBound)
  hybrid SteepToSixtyUp (UpSteep->UpSixty): 26.116–39.430 -> 26.116–39.300 (min +0.000; max -0.130; sources EnergyMinimum/ForceBound -> EnergyMinimum/ForceBound)
  hybrid UnbankLeft (DownGentle->DownGentle): 5.000–24.580 -> 9.370–24.580 (min +4.370; max +0.000; sources EnergyMinimum/ForceBound -> ApproachMinimum/ForceBound)
  hybrid UnbankLeft (UpGentle->UpGentle): 17.710–28.540 -> 17.750–28.370 (min +0.040; max -0.170; sources ClearanceMinimum/ForceBound -> ClearanceMinimum/ForceBound)
  hybrid UnbankRight (DownGentle->DownGentle): 5.000–24.580 -> 9.370–24.580 (min +4.370; max +0.000; sources EnergyMinimum/ForceBound -> ApproachMinimum/ForceBound)
  hybrid UnbankRight (UpGentle->UpGentle): 17.710–28.540 -> 17.750–28.370 (min +0.040; max -0.170; sources ClearanceMinimum/ForceBound -> ClearanceMinimum/ForceBound)
  hyper BankLeft (DownGentle->DownGentle): 5.000–19.130 -> 5.000–18.510 (min +0.000; max -0.620; sources EnergyMinimum/ForceBound -> EnergyMinimum/ForceBound)
  hyper BankLeft (Level->Level): 5.000–18.390 -> 5.000–18.400 (min +0.000; max +0.010; sources EnergyMinimum/ForceBound -> EnergyMinimum/ForceBound)
  hyper BankLeft (UpGentle->UpGentle): 17.670–26.900 -> 18.320–26.890 (min +0.650; max -0.010; sources ClearanceMinimum/ForceBound -> ClearanceMinimum/ForceBound)
  hyper BankRight (DownGentle->DownGentle): 5.000–19.130 -> 5.000–18.510 (min +0.000; max -0.620; sources EnergyMinimum/ForceBound -> EnergyMinimum/ForceBound)
  hyper BankRight (Level->Level): 5.000–18.390 -> 5.000–18.400 (min +0.000; max +0.010; sources EnergyMinimum/ForceBound -> EnergyMinimum/ForceBound)
  hyper BankRight (UpGentle->UpGentle): 17.670–26.900 -> 18.320–26.890 (min +0.650; max -0.010; sources ClearanceMinimum/ForceBound -> ClearanceMinimum/ForceBound)
  hyper BankedStraightLeft (DownGentle->DownGentle): 5.000–30.000 -> 9.230–22.760 (min +4.230; max -7.240; sources EnergyMinimum/DesignCap -> ApproachMinimum/ForceBound)
  hyper BankedStraightLeft (Level->Level): 5.000–30.000 -> 5.000–18.400 (min +0.000; max -11.600; sources EnergyMinimum/DesignCap -> EnergyMinimum/ForceBound)
  hyper BankedStraightLeft (UpGentle->UpGentle): 20.000–30.000 -> 20.000–24.990 (min +0.000; max -5.010; sources EnergyMinimum/DesignCap -> EnergyMinimum/ForceBound)
  hyper BankedStraightRight (DownGentle->DownGentle): 5.000–30.000 -> 9.230–22.760 (min +4.230; max -7.240; sources EnergyMinimum/DesignCap -> ApproachMinimum/ForceBound)
  hyper BankedStraightRight (Level->Level): 5.000–30.000 -> 5.000–18.400 (min +0.000; max -11.600; sources EnergyMinimum/DesignCap -> EnergyMinimum/ForceBound)
  hyper BankedStraightRight (UpGentle->UpGentle): 20.000–30.000 -> 20.000–24.990 (min +0.000; max -5.010; sources EnergyMinimum/DesignCap -> EnergyMinimum/ForceBound)
  hyper SixtyDownToLevel (DownSixty->Level): 2.000–20.640 -> 12.590–20.620 (min +10.590; max -0.020; sources EnergyMinimum/ForceBound -> ApproachMinimum/ForceBound)
  hyper SixtyToSteepDown (DownSixty->DownSteep): 2.000–27.640 -> 12.590–27.620 (min +10.590; max -0.020; sources EnergyMinimum/ForceBound -> ApproachMinimum/ForceBound)
  hyper SteepToSixtyDown (DownSteep->DownSixty): 3.000–24.600 -> 11.780–24.660 (min +8.780; max +0.060; sources EnergyMinimum/ForceBound -> ApproachMinimum/ForceBound)
  hyper UnbankLeft (DownGentle->DownGentle): 5.000–20.170 -> 9.370–20.170 (min +4.370; max +0.000; sources EnergyMinimum/ForceBound -> ApproachMinimum/ForceBound)
  hyper UnbankLeft (UpGentle->UpGentle): 18.320–25.120 -> 18.320–25.000 (min +0.000; max -0.120; sources ClearanceMinimum/ForceBound -> ClearanceMinimum/ForceBound)
  hyper UnbankRight (DownGentle->DownGentle): 5.000–20.170 -> 9.370–20.170 (min +4.370; max +0.000; sources EnergyMinimum/ForceBound -> ApproachMinimum/ForceBound)
  hyper UnbankRight (UpGentle->UpGentle): 18.320–25.120 -> 18.320–25.000 (min +0.000; max -0.120; sources ClearanceMinimum/ForceBound -> ClearanceMinimum/ForceBound)
  hyper WideSixtyDownToLevel (DownSixty->Level): 2.000–26.790 -> 12.590–26.760 (min +10.590; max -0.030; sources EnergyMinimum/ForceBound -> ApproachMinimum/ForceBound)
  velocity_viper BankLeft (DownGentle->DownGentle): 5.000–23.760 -> 5.250–23.430 (min +0.250; max -0.330; sources EnergyMinimum/ForceBound -> ApproachMinimum/ForceBound)
  velocity_viper BankLeft (UpGentle->UpGentle): 17.240–30.180 -> 17.750–30.190 (min +0.510; max +0.010; sources ClearanceMinimum/ForceBound -> ClearanceMinimum/ForceBound)
  velocity_viper BankRight (DownGentle->DownGentle): 5.000–23.760 -> 5.250–23.430 (min +0.250; max -0.330; sources EnergyMinimum/ForceBound -> ApproachMinimum/ForceBound)
  velocity_viper BankRight (UpGentle->UpGentle): 17.240–30.180 -> 17.750–30.190 (min +0.510; max +0.010; sources ClearanceMinimum/ForceBound -> ClearanceMinimum/ForceBound)
  velocity_viper BankedStraightLeft (DownGentle->DownGentle): 5.000–30.000 -> 9.320–26.690 (min +4.320; max -3.310; sources EnergyMinimum/DesignCap -> ApproachMinimum/ForceBound)
  velocity_viper BankedStraightLeft (Level->Level): 5.000–30.000 -> 5.000–26.120 (min +0.000; max -3.880; sources EnergyMinimum/DesignCap -> EnergyMinimum/ForceBound)
  velocity_viper BankedStraightLeft (UpGentle->UpGentle): 20.000–30.000 -> 20.000–28.000 (min +0.000; max -2.000; sources EnergyMinimum/DesignCap -> EnergyMinimum/ForceBound)
  velocity_viper BankedStraightRight (DownGentle->DownGentle): 5.000–30.000 -> 9.320–26.690 (min +4.320; max -3.310; sources EnergyMinimum/DesignCap -> ApproachMinimum/ForceBound)
  velocity_viper BankedStraightRight (Level->Level): 5.000–30.000 -> 5.000–26.120 (min +0.000; max -3.880; sources EnergyMinimum/DesignCap -> EnergyMinimum/ForceBound)
  velocity_viper BankedStraightRight (UpGentle->UpGentle): 20.000–30.000 -> 20.000–28.000 (min +0.000; max -2.000; sources EnergyMinimum/DesignCap -> EnergyMinimum/ForceBound)
  velocity_viper EjectorHill8x48 (Level->Level): 13.199–19.650 -> 13.199–19.660 (min +0.000; max +0.010; sources EnergyMinimum/AirtimeFeel -> EnergyMinimum/AirtimeFeel)
  velocity_viper FamilyHill4x40 (Level->Level): 9.967–17.280 -> 9.967–17.290 (min +0.000; max +0.010; sources EnergyMinimum/AirtimeFeel -> EnergyMinimum/AirtimeFeel)
  velocity_viper HalfCorkscrewLeftDown (Level->Level): 5.000–15.766 -> 5.000–15.760 (min +0.000; max -0.006; sources EnergyMinimum/DesignCap -> EnergyMinimum/ClearanceMaximum)
  velocity_viper HalfCorkscrewRightDown (Level->Level): 5.000–15.766 -> 5.000–15.760 (min +0.000; max -0.006; sources EnergyMinimum/DesignCap -> EnergyMinimum/ClearanceMaximum)
  velocity_viper HalfLoopDown (Level->Level): 4.000–22.240 -> 4.000–21.900 (min +0.000; max -0.340; sources EnergyMinimum/ForceBound -> EnergyMinimum/ForceBound)
  velocity_viper RavenRollLeftDown (Level->Level): 8.000–20.210 -> 8.000–19.840 (min +0.000; max -0.370; sources EnergyMinimum/ForceBound -> EnergyMinimum/ForceBound)
  velocity_viper RavenRollLeftUp (Level->Level): 8.000–18.320 -> 8.000–18.330 (min +0.000; max +0.010; sources EnergyMinimum/ForceBound -> EnergyMinimum/ForceBound)
  velocity_viper RavenRollRightDown (Level->Level): 8.000–20.210 -> 8.000–19.840 (min +0.000; max -0.370; sources EnergyMinimum/ForceBound -> EnergyMinimum/ForceBound)
  velocity_viper RavenRollRightUp (Level->Level): 8.000–18.320 -> 8.000–18.330 (min +0.000; max +0.010; sources EnergyMinimum/ForceBound -> EnergyMinimum/ForceBound)
  velocity_viper RolloutLeft (DownGentle->DownGentle): 5.000–52.660 -> 9.370–52.670 (min +4.370; max +0.010; sources EnergyMinimum/ForceBound -> ApproachMinimum/ForceBound)
  velocity_viper RolloutLeft (UpGentle->UpGentle): 22.000–55.340 -> 22.000–55.290 (min +0.000; max -0.050; sources EnergyMinimum/ForceBound -> EnergyMinimum/ForceBound)
  velocity_viper RolloutRight (DownGentle->DownGentle): 5.000–52.660 -> 9.370–52.670 (min +4.370; max +0.010; sources EnergyMinimum/ForceBound -> ApproachMinimum/ForceBound)
  velocity_viper RolloutRight (UpGentle->UpGentle): 22.000–55.340 -> 22.000–55.290 (min +0.000; max -0.050; sources EnergyMinimum/ForceBound -> EnergyMinimum/ForceBound)
  velocity_viper SixtyDownToLevel (DownSixty->Level): 2.000–28.500 -> 12.560–28.480 (min +10.560; max -0.020; sources EnergyMinimum/ForceBound -> ApproachMinimum/ForceBound)
  velocity_viper SixtyToSteepDown (DownSixty->DownSteep): 2.000–33.300 -> 12.560–33.280 (min +10.560; max -0.020; sources EnergyMinimum/ForceBound -> ApproachMinimum/ForceBound)
  velocity_viper SixtyToSteepUp (UpSixty->UpSteep): 27.000–38.100 -> 27.000–37.960 (min +0.000; max -0.140; sources EnergyMinimum/ForceBound -> EnergyMinimum/ForceBound)
  velocity_viper SteepToSixtyDown (DownSteep->DownSixty): 3.000–27.920 -> 10.510–27.940 (min +7.510; max +0.020; sources EnergyMinimum/ForceBound -> ApproachMinimum/ForceBound)
  velocity_viper SteepToSixtyUp (UpSteep->UpSixty): 26.116–41.360 -> 26.116–41.240 (min +0.000; max -0.120; sources EnergyMinimum/ForceBound -> EnergyMinimum/ForceBound)
  velocity_viper UnbankLeft (DownGentle->DownGentle): 5.000–24.580 -> 9.370–24.580 (min +4.370; max +0.000; sources EnergyMinimum/ForceBound -> ApproachMinimum/ForceBound)
  velocity_viper UnbankLeft (UpGentle->UpGentle): 17.710–28.540 -> 17.750–28.370 (min +0.040; max -0.170; sources ClearanceMinimum/ForceBound -> ClearanceMinimum/ForceBound)
  velocity_viper UnbankRight (DownGentle->DownGentle): 5.000–24.580 -> 9.370–24.580 (min +4.370; max +0.000; sources EnergyMinimum/ForceBound -> ApproachMinimum/ForceBound)
  velocity_viper UnbankRight (UpGentle->UpGentle): 17.710–28.540 -> 17.750–28.370 (min +0.040; max -0.170; sources ClearanceMinimum/ForceBound -> ClearanceMinimum/ForceBound)
  wooden_twister BankLeft (DownGentle->DownGentle): 5.000–23.550 -> 5.000–23.140 (min +0.000; max -0.410; sources EnergyMinimum/ForceBound -> EnergyMinimum/ForceBound)
  wooden_twister BankLeft (UpGentle->UpGentle): 17.530–30.130 -> 18.080–30.130 (min +0.550; max +0.000; sources ClearanceMinimum/ForceBound -> ClearanceMinimum/ForceBound)
  wooden_twister BankRight (DownGentle->DownGentle): 5.000–23.550 -> 5.000–23.140 (min +0.000; max -0.410; sources EnergyMinimum/ForceBound -> EnergyMinimum/ForceBound)
  wooden_twister BankRight (UpGentle->UpGentle): 17.530–30.130 -> 18.080–30.130 (min +0.550; max +0.000; sources ClearanceMinimum/ForceBound -> ClearanceMinimum/ForceBound)
  wooden_twister BankedStraightLeft (DownGentle->DownGentle): 5.000–30.000 -> 9.290–26.370 (min +4.290; max -3.630; sources EnergyMinimum/DesignCap -> ApproachMinimum/ForceBound)
  wooden_twister BankedStraightLeft (Level->Level): 5.000–30.000 -> 5.000–26.120 (min +0.000; max -3.880; sources EnergyMinimum/DesignCap -> EnergyMinimum/ForceBound)
  wooden_twister BankedStraightLeft (UpGentle->UpGentle): 20.000–30.000 -> 20.000–28.000 (min +0.000; max -2.000; sources EnergyMinimum/DesignCap -> EnergyMinimum/ForceBound)
  wooden_twister BankedStraightRight (DownGentle->DownGentle): 5.000–30.000 -> 9.290–26.370 (min +4.290; max -3.630; sources EnergyMinimum/DesignCap -> ApproachMinimum/ForceBound)
  wooden_twister BankedStraightRight (Level->Level): 5.000–30.000 -> 5.000–26.120 (min +0.000; max -3.880; sources EnergyMinimum/DesignCap -> EnergyMinimum/ForceBound)
  wooden_twister BankedStraightRight (UpGentle->UpGentle): 20.000–30.000 -> 20.000–28.000 (min +0.000; max -2.000; sources EnergyMinimum/DesignCap -> EnergyMinimum/ForceBound)
  wooden_twister RolloutLeft (DownGentle->DownGentle): 5.000–52.540 -> 9.370–52.540 (min +4.370; max +0.000; sources EnergyMinimum/ForceBound -> ApproachMinimum/ForceBound)
  wooden_twister RolloutLeft (UpGentle->UpGentle): 22.000–55.340 -> 22.000–55.290 (min +0.000; max -0.050; sources EnergyMinimum/ForceBound -> EnergyMinimum/ForceBound)
  wooden_twister RolloutRight (DownGentle->DownGentle): 5.000–52.540 -> 9.370–52.540 (min +4.370; max +0.000; sources EnergyMinimum/ForceBound -> ApproachMinimum/ForceBound)
  wooden_twister RolloutRight (UpGentle->UpGentle): 22.000–55.340 -> 22.000–55.290 (min +0.000; max -0.050; sources EnergyMinimum/ForceBound -> EnergyMinimum/ForceBound)
  wooden_twister UnbankLeft (DownGentle->DownGentle): 5.000–24.320 -> 9.370–24.320 (min +4.370; max +0.000; sources EnergyMinimum/ForceBound -> ApproachMinimum/ForceBound)
  wooden_twister UnbankLeft (UpGentle->UpGentle): 18.080–28.540 -> 18.080–28.370 (min +0.000; max -0.170; sources ClearanceMinimum/ForceBound -> ClearanceMinimum/ForceBound)
  wooden_twister UnbankRight (DownGentle->DownGentle): 5.000–24.320 -> 9.370–24.320 (min +4.370; max +0.000; sources EnergyMinimum/ForceBound -> ApproachMinimum/ForceBound)
  wooden_twister UnbankRight (UpGentle->UpGentle): 18.080–28.540 -> 18.080–28.370 (min +0.000; max -0.170; sources ClearanceMinimum/ForceBound -> ClearanceMinimum/ForceBound)
Changed measured windows: 116
Context probes for bound changes over 1.5 m/s: 64
```

## Contexts at changes over 1.5 m/s

Probes are 0.01 m/s outside the new bound. A no-approach result is a limitation of the unpowered pre-roll, not a measured force violation.

```text
PROBE family_coaster BankedStraightLeft DownGentle->DownGentle new_max_plus_0.01 17.170 m/s: pass=false, limiting=Gentle down, bank left, banked straight, then left rollout, failure=None
PROBE family_coaster BankedStraightLeft DownGentle->DownGentle new_min_minus_0.01 9.340 m/s: pass=false, limiting=connected model-allowed approach, failure=Some("no approach context can attain the requested entry speed")
PROBE family_coaster BankedStraightLeft Level->Level new_max_plus_0.01 14.820 m/s: pass=false, limiting=Bank left then banked straight into left rollout, failure=None
PROBE family_coaster BankedStraightRight DownGentle->DownGentle new_max_plus_0.01 17.170 m/s: pass=false, limiting=Gentle down, bank right, banked straight, then right rollout, failure=None
PROBE family_coaster BankedStraightRight DownGentle->DownGentle new_min_minus_0.01 9.340 m/s: pass=false, limiting=connected model-allowed approach, failure=Some("no approach context can attain the requested entry speed")
PROBE family_coaster BankedStraightRight Level->Level new_max_plus_0.01 14.820 m/s: pass=false, limiting=Bank right then banked straight into right rollout, failure=None
PROBE family_coaster UnbankLeft DownGentle->DownGentle new_min_minus_0.01 9.360 m/s: pass=false, limiting=connected model-allowed approach, failure=Some("no approach context can attain the requested entry speed")
PROBE family_coaster UnbankRight DownGentle->DownGentle new_min_minus_0.01 9.360 m/s: pass=false, limiting=connected model-allowed approach, failure=Some("no approach context can attain the requested entry speed")
PROBE hybrid BankedStraightLeft DownGentle->DownGentle new_max_plus_0.01 26.700 m/s: pass=false, limiting=Gentle down, bank left, banked straight, then left rollout, failure=None
PROBE hybrid BankedStraightLeft DownGentle->DownGentle new_min_minus_0.01 9.310 m/s: pass=false, limiting=connected model-allowed approach, failure=Some("no approach context can attain the requested entry speed")
PROBE hybrid BankedStraightLeft Level->Level new_max_plus_0.01 26.130 m/s: pass=false, limiting=Bank left then banked straight into left rollout, failure=None
PROBE hybrid BankedStraightLeft UpGentle->UpGentle new_max_plus_0.01 28.010 m/s: pass=false, limiting=Gentle up, bank left, banked straight, then left rollout, failure=None
PROBE hybrid BankedStraightRight DownGentle->DownGentle new_max_plus_0.01 26.700 m/s: pass=false, limiting=Gentle down, bank right, banked straight, then right rollout, failure=None
PROBE hybrid BankedStraightRight DownGentle->DownGentle new_min_minus_0.01 9.310 m/s: pass=false, limiting=connected model-allowed approach, failure=Some("no approach context can attain the requested entry speed")
PROBE hybrid BankedStraightRight Level->Level new_max_plus_0.01 26.130 m/s: pass=false, limiting=Bank right then banked straight into right rollout, failure=None
PROBE hybrid BankedStraightRight UpGentle->UpGentle new_max_plus_0.01 28.010 m/s: pass=false, limiting=Gentle up, bank right, banked straight, then right rollout, failure=None
PROBE hybrid RolloutLeft DownGentle->DownGentle new_min_minus_0.01 9.360 m/s: pass=false, limiting=connected model-allowed approach, failure=Some("no approach context can attain the requested entry speed")
PROBE hybrid RolloutRight DownGentle->DownGentle new_min_minus_0.01 9.360 m/s: pass=false, limiting=connected model-allowed approach, failure=Some("no approach context can attain the requested entry speed")
PROBE hybrid SixtyDownToLevel DownSixty->Level new_min_minus_0.01 12.550 m/s: pass=false, limiting=connected model-allowed approach, failure=Some("no approach context can attain the requested entry speed")
PROBE hybrid SixtyToSteepDown DownSixty->DownSteep new_min_minus_0.01 12.550 m/s: pass=false, limiting=connected model-allowed approach, failure=Some("no approach context can attain the requested entry speed")
PROBE hybrid SteepToSixtyDown DownSteep->DownSixty new_min_minus_0.01 10.500 m/s: pass=false, limiting=connected model-allowed approach, failure=Some("no approach context can attain the requested entry speed")
PROBE hybrid UnbankLeft DownGentle->DownGentle new_min_minus_0.01 9.360 m/s: pass=false, limiting=connected model-allowed approach, failure=Some("no approach context can attain the requested entry speed")
PROBE hybrid UnbankRight DownGentle->DownGentle new_min_minus_0.01 9.360 m/s: pass=false, limiting=connected model-allowed approach, failure=Some("no approach context can attain the requested entry speed")
PROBE hyper BankedStraightLeft DownGentle->DownGentle new_max_plus_0.01 22.770 m/s: pass=false, limiting=Gentle down, bank left, banked straight, then left rollout, failure=None
PROBE hyper BankedStraightLeft DownGentle->DownGentle new_min_minus_0.01 9.220 m/s: pass=false, limiting=connected model-allowed approach, failure=Some("no approach context can attain the requested entry speed")
PROBE hyper BankedStraightLeft Level->Level new_max_plus_0.01 18.410 m/s: pass=false, limiting=Bank left then banked straight into left rollout, failure=None
PROBE hyper BankedStraightLeft UpGentle->UpGentle new_max_plus_0.01 25.000 m/s: pass=false, limiting=Gentle up, bank left, banked straight, then left rollout, failure=None
PROBE hyper BankedStraightRight DownGentle->DownGentle new_max_plus_0.01 22.770 m/s: pass=false, limiting=Gentle down, bank right, banked straight, then right rollout, failure=None
PROBE hyper BankedStraightRight DownGentle->DownGentle new_min_minus_0.01 9.220 m/s: pass=false, limiting=connected model-allowed approach, failure=Some("no approach context can attain the requested entry speed")
PROBE hyper BankedStraightRight Level->Level new_max_plus_0.01 18.410 m/s: pass=false, limiting=Bank right then banked straight into right rollout, failure=None
PROBE hyper BankedStraightRight UpGentle->UpGentle new_max_plus_0.01 25.000 m/s: pass=false, limiting=Gentle up, bank right, banked straight, then right rollout, failure=None
PROBE hyper SixtyDownToLevel DownSixty->Level new_min_minus_0.01 12.580 m/s: pass=false, limiting=connected model-allowed approach, failure=Some("no approach context can attain the requested entry speed")
PROBE hyper SixtyToSteepDown DownSixty->DownSteep new_min_minus_0.01 12.580 m/s: pass=false, limiting=connected model-allowed approach, failure=Some("no approach context can attain the requested entry speed")
PROBE hyper SteepToSixtyDown DownSteep->DownSixty new_min_minus_0.01 11.770 m/s: pass=false, limiting=connected model-allowed approach, failure=Some("no approach context can attain the requested entry speed")
PROBE hyper UnbankLeft DownGentle->DownGentle new_min_minus_0.01 9.360 m/s: pass=false, limiting=connected model-allowed approach, failure=Some("no approach context can attain the requested entry speed")
PROBE hyper UnbankRight DownGentle->DownGentle new_min_minus_0.01 9.360 m/s: pass=false, limiting=connected model-allowed approach, failure=Some("no approach context can attain the requested entry speed")
PROBE hyper WideSixtyDownToLevel DownSixty->Level new_min_minus_0.01 12.580 m/s: pass=false, limiting=connected model-allowed approach, failure=Some("no approach context can attain the requested entry speed")
PROBE velocity_viper BankedStraightLeft DownGentle->DownGentle new_max_plus_0.01 26.700 m/s: pass=false, limiting=Gentle down, bank left, banked straight, then left rollout, failure=None
PROBE velocity_viper BankedStraightLeft DownGentle->DownGentle new_min_minus_0.01 9.310 m/s: pass=false, limiting=connected model-allowed approach, failure=Some("no approach context can attain the requested entry speed")
PROBE velocity_viper BankedStraightLeft Level->Level new_max_plus_0.01 26.130 m/s: pass=false, limiting=Bank left then banked straight into left rollout, failure=None
PROBE velocity_viper BankedStraightLeft UpGentle->UpGentle new_max_plus_0.01 28.010 m/s: pass=false, limiting=Gentle up, bank left, banked straight, then left rollout, failure=None
PROBE velocity_viper BankedStraightRight DownGentle->DownGentle new_max_plus_0.01 26.700 m/s: pass=false, limiting=Gentle down, bank right, banked straight, then right rollout, failure=None
PROBE velocity_viper BankedStraightRight DownGentle->DownGentle new_min_minus_0.01 9.310 m/s: pass=false, limiting=connected model-allowed approach, failure=Some("no approach context can attain the requested entry speed")
PROBE velocity_viper BankedStraightRight Level->Level new_max_plus_0.01 26.130 m/s: pass=false, limiting=Bank right then banked straight into right rollout, failure=None
PROBE velocity_viper BankedStraightRight UpGentle->UpGentle new_max_plus_0.01 28.010 m/s: pass=false, limiting=Gentle up, bank right, banked straight, then right rollout, failure=None
PROBE velocity_viper RolloutLeft DownGentle->DownGentle new_min_minus_0.01 9.360 m/s: pass=false, limiting=connected model-allowed approach, failure=Some("no approach context can attain the requested entry speed")
PROBE velocity_viper RolloutRight DownGentle->DownGentle new_min_minus_0.01 9.360 m/s: pass=false, limiting=connected model-allowed approach, failure=Some("no approach context can attain the requested entry speed")
PROBE velocity_viper SixtyDownToLevel DownSixty->Level new_min_minus_0.01 12.550 m/s: pass=false, limiting=connected model-allowed approach, failure=Some("no approach context can attain the requested entry speed")
PROBE velocity_viper SixtyToSteepDown DownSixty->DownSteep new_min_minus_0.01 12.550 m/s: pass=false, limiting=connected model-allowed approach, failure=Some("no approach context can attain the requested entry speed")
PROBE velocity_viper SteepToSixtyDown DownSteep->DownSixty new_min_minus_0.01 10.500 m/s: pass=false, limiting=connected model-allowed approach, failure=Some("no approach context can attain the requested entry speed")
PROBE velocity_viper UnbankLeft DownGentle->DownGentle new_min_minus_0.01 9.360 m/s: pass=false, limiting=connected model-allowed approach, failure=Some("no approach context can attain the requested entry speed")
PROBE velocity_viper UnbankRight DownGentle->DownGentle new_min_minus_0.01 9.360 m/s: pass=false, limiting=connected model-allowed approach, failure=Some("no approach context can attain the requested entry speed")
PROBE wooden_twister BankedStraightLeft DownGentle->DownGentle new_max_plus_0.01 26.380 m/s: pass=false, limiting=Gentle down, bank left, banked straight, then left rollout, failure=None
PROBE wooden_twister BankedStraightLeft DownGentle->DownGentle new_min_minus_0.01 9.280 m/s: pass=false, limiting=connected model-allowed approach, failure=Some("no approach context can attain the requested entry speed")
PROBE wooden_twister BankedStraightLeft Level->Level new_max_plus_0.01 26.130 m/s: pass=false, limiting=Bank left then banked straight into left rollout, failure=None
PROBE wooden_twister BankedStraightLeft UpGentle->UpGentle new_max_plus_0.01 28.010 m/s: pass=false, limiting=Gentle up, bank left, banked straight, then left rollout, failure=None
PROBE wooden_twister BankedStraightRight DownGentle->DownGentle new_max_plus_0.01 26.380 m/s: pass=false, limiting=Gentle down, bank right, banked straight, then right rollout, failure=None
PROBE wooden_twister BankedStraightRight DownGentle->DownGentle new_min_minus_0.01 9.280 m/s: pass=false, limiting=connected model-allowed approach, failure=Some("no approach context can attain the requested entry speed")
PROBE wooden_twister BankedStraightRight Level->Level new_max_plus_0.01 26.130 m/s: pass=false, limiting=Bank right then banked straight into right rollout, failure=None
PROBE wooden_twister BankedStraightRight UpGentle->UpGentle new_max_plus_0.01 28.010 m/s: pass=false, limiting=Gentle up, bank right, banked straight, then right rollout, failure=None
PROBE wooden_twister RolloutLeft DownGentle->DownGentle new_min_minus_0.01 9.360 m/s: pass=false, limiting=connected model-allowed approach, failure=Some("no approach context can attain the requested entry speed")
PROBE wooden_twister RolloutRight DownGentle->DownGentle new_min_minus_0.01 9.360 m/s: pass=false, limiting=connected model-allowed approach, failure=Some("no approach context can attain the requested entry speed")
PROBE wooden_twister UnbankLeft DownGentle->DownGentle new_min_minus_0.01 9.360 m/s: pass=false, limiting=connected model-allowed approach, failure=Some("no approach context can attain the requested entry speed")
PROBE wooden_twister UnbankRight DownGentle->DownGentle new_min_minus_0.01 9.360 m/s: pass=false, limiting=connected model-allowed approach, failure=Some("no approach context can attain the requested entry speed")
```
