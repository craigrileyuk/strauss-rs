# Benchmarks

These are timings of `prefix` for Strauss PHP and Strauss-RS on four projects of different sizes. They were taken on one machine, so treat them as a guide to the size of the difference. Your results will vary with your hardware, your project and your disk.

## Results

Each time is the median of several runs: seven for the small and medium projects, three for the large and extra large ones.

| Project                                  | Packages | PHP files | Strauss PHP 0.31.1 | Strauss-RS | Strauss-RS, single-thread |
| ---------------------------------------- | -------: | --------: | -----------------: | ---------: | ------------------------: |
| Small (`monolog`)                        |        2 |       142 |             0.82 s |    0.012 s |                   0.025 s |
| Medium (`guzzle`, `flysystem`, `finder`) |       12 |       266 |             1.52 s |    0.018 s |                   0.054 s |
| Large (`laravel/framework`)              |       73 |     5,369 |             19.3 s |     0.18 s |                    0.73 s |
| Extra large (`aws/aws-sdk-php`)          |       15 |     3,769 |             47.0 s |     0.36 s |                    1.40 s |

These figures roughly work out at the following:

| Project     |   Strauss-RS | Strauss-RS single-threaded |
| ----------- | -----------: | -------------------------: |
| Small       |  ~68x faster |                ~33x faster |
| Medium      |  ~84x faster |                ~28x faster |
| Large       | ~110x faster |                ~26x faster |
| Extra large | ~130x faster |                ~34x faster |

## How they were run

- PHP 8.4.26, with OPCache off and no memory limit.
- A system with 12 threads. The CPU model, memory, operating system and disk are not recorded.
- Strauss PHP 0.31.1 and Strauss-RS were run on the same four projects.

## What to keep in mind

- The times are short for the small and medium projects, so a few milliseconds of start-up time makes a large difference to the ratio. The ratios for those projects are the least reliable.
- These measure the `prefix` step. A Composer install and your own build steps take the same time with either tool.
