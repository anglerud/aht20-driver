# Contributors

See the <CONTRIBUTING.md> file for guidelines around contributing to this
project.


## Special thanks to everyone who's helped out the project

* [Max Barnash (arr-ee)](https://github.com/arr-ee) for troubleshooting and
  insights around the driver's Check Status behavior, and for the patch! 
* [Samuel Holland (smaeul)](https://github.com/smaeul) for spotting and
  patching an error in the humidity readings.
* [Andrew Straw (astraw)](https://github.com/astraw) for several highly
  appreciated contributions such as:
  - the defmt feature flag
  - updating the project to support the embedded HAL 1.0
  - implementing `core::error::Error` for our `aht20_driver::Error<E>`
