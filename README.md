Home Data Logger
================

A work in progress data logger for home weather data running on nRF52840.

TODO: Add more info

Testing
-------

    cargo host-test

Notes
-----

From bluetoothctl:

    Discovery started
    [CHG] Controller 3C:F0:11:DB:B9:F0 Discovering: yes
    [NEW] Device 69:0E:AE:2C:5C:AB 69-0E-AE-2C-5C-AB
    [NEW] Device 45:5A:70:F1:56:4C 45-5A-70-F1-56-4C
    [NEW] Device 49:84:BE:31:50:6F 49-84-BE-31-50-6F
    [NEW] Device 5D:AD:3B:BA:35:B2 5D-AD-3B-BA-35-B2
    [NEW] Device 51:D4:4A:33:41:B9 51-D4-4A-33-41-B9
    [NEW] Device A4:C1:38:59:BE:24 GVH5075_BE24
    [NEW] Device 90:9C:4A:C9:8A:04 90-9C-4A-C9-8A-04
    [NEW] Device 57:09:53:54:0D:C7 57-09-53-54-0D-C7
    [CHG] Device 69:0E:AE:2C:5C:AB ManufacturerData Key: 0x004c
    [CHG] Device 69:0E:AE:2C:5C:AB ManufacturerData Value:
    01 01 00 00 00 00 00 00 00 00 00 00 00 00 00 00  ................
    00                                               .
    [CHG] Device A4:C1:38:59:BE:24 RSSI: -77
    [CHG] Device A4:C1:38:59:BE:24 ManufacturerData Key: 0xec88
    [CHG] Device A4:C1:38:59:BE:24 ManufacturerData Value:
    00 03 36 ed 3a 00                                ..6.:.
    [CHG] Device A4:C1:38:59:BE:24 ManufacturerData Key: 0x004c
    [CHG] Device A4:C1:38:59:BE:24 ManufacturerData Value:
    02 15 49 4e 54 45 4c 4c 49 5f 52 4f 43 4b 53 5f  ..INTELLI_ROCKS_
    48 57 50 75 f2 ff c2                             HWPu...
    [NEW] Device E3:37:3C:50:EC:4E Govee_H5074_EC4E
    [CHG] Device 69:0E:AE:2C:5C:AB ManufacturerData Key: 0x004c
    [CHG] Device 69:0E:AE:2C:5C:AB ManufacturerData Value:
    01 01 00 00 00 00 00 00 00 00 00 00 00 00 00 00  ................
    00                                               .
    [CHG] Device 49:84:BE:31:50:6F RSSI: -87
    [CHG] Device A4:C1:38:59:BE:24 RSSI: -54
    [CHG] Device A4:C1:38:59:BE:24 ManufacturerData Key: 0xec88
    [CHG] Device A4:C1:38:59:BE:24 ManufacturerData Value:
    00 03 33 04 3a 00                                ..3.:.
    [CHG] Device A4:C1:38:59:BE:24 ManufacturerData Key: 0x004c
    [CHG] Device A4:C1:38:59:BE:24 ManufacturerData Value:
    02 15 49 4e 54 45 4c 4c 49 5f 52 4f 43 4b 53 5f  ..INTELLI_ROCKS_
    48 57 50 75 f2 ff c2                             HWPu...
    [CHG] Device E3:37:3C:50:EC:4E ManufacturerData Key: 0xec88
    [CHG] Device E3:37:3C:50:EC:4E ManufacturerData Value:
    00 83 06 b5 22 64 02                             ...."d.
    [CHG] Device 49:84:BE:31:50:6F RSSI: -74
    [CHG] Device 90:9C:4A:C9:8A:04 RSSI: -86
    [CHG] Device 69:0E:AE:2C:5C:AB ManufacturerData Key: 0x004c
    [CHG] Device 69:0E:AE:2C:5C:AB ManufacturerData Value:
    01 01 00 00 00 00 00 00 00 00 00 00 00 00 00 00  ................
    00                                               .

These are the two of interest:

* [NEW] Device A4:C1:38:59:BE:24 GVH5075_BE24
* [NEW] Device E3:37:3C:50:EC:4E Govee_H5074_EC4E

License
-------

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  http://www.apache.org/licenses/LICENSE-2.0)

- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

Contribution
------------

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall
be licensed as above, without any additional terms or conditions.
