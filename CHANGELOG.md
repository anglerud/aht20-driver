# Changelog

All notable changes to this project will be documented in this file.

This format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).


## [Unreleased]


## [2.0.0] - 2024-11-01

### Changed

* Upgrade to the embedded HAL 1.0 release. Thank you to [Andrew Straw
  (astraw)](https://github.com/astraw) for the addition.
* Impement `core::error::Error` for our `aht20_driver::Error<E>`, again thank
  you to Andrew for this work. Note that this increases our Minimum Supported
  Rust Version to 1.81.


## [1.2.2] - 2024-10-21

### Added

* A feature flag for defmt. Thank you to [Andrew Straw
  (astraw)](https://github.com/astraw) for the addition.


### Fixed

* An error in the humidity calculation was fixed by [Samuel Holland
  (smaeul)](https://github.com/smaeul).


## [1.2.1] - 2024-04-28

### Fixed

* CheckStatus behavior that could cause a hang. Thank you to [Max Barnash
  (arr-ee)](https://github.com/arr-ee) for the report and patch.


## [1.2.0] - 2022-02-22

### Added

* The `measure_no_fp` method that takes a measurement, but does not use
  floating point math.


## [1.1.0] - 2022-02-22

### Added

* Added this changelog.


### Changed

* Changed from rtt to Defmt logging, saving more than 30k of flash space.


[Unreleased]: https://github.com/anglerud/aht20-driver/compare/v1.2.1...HEAD
[1.2.1]: https://github.com/anglerud/aht20-driver/compare/v1.2.0...v1.2.1
[1.2.0]: https://github.com/anglerud/aht20-driver/compare/v1.1.0...v1.2.0
[1.1.0]: https://github.com/anglerud/aht20-driver/compare/v1.0.0...v1.1.0
