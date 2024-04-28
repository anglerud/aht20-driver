# Changelog

All notable changes to this project will be documented in this file.

This format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).


## [Unreleased]

* Added continuous integration via Github actions


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
