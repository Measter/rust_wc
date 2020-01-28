# RustWC

A Rust implementation of the WC tool.

## Example output

![rust_wc](https://i.imgur.com/YPyexHY.png)

## Why?

This was written as a learning exercise to discover how much performance an (at best) average programmer could obtain
relative to the C version (GNU Coreutils 8.30) on my test system. As such, optimisations are limited to "low hanging
fruit": querying the file system for file size, putting the more expensive counts behind a flag, and parallelising file
handling.

Unlike some other comparisons I've seen, I've attempted to be reasonably feature complete in order to get a fair
comparison. It's hardly fair to compare a program which handles 1 file, with no stdin support, and no command line
options to a full program with all of these features.

## Differences

This is not a complete clone of the C program. There are differences. This program *only* supports UTF-8 input, while
the C version supports different encodings based on an environment setting. That could result in differences in the speed
of processing.

My implementation uses the default (as of 2020-01-28) 8kB buffer, while the C version uses a 16kB buffer. I did try a 16kB buffer and didn't notice
any noticeable change in execution speed, so I reverted back to the default.

The C version processes that buffer directly, while my implementation copies a line of text into a second buffer (`line_buf`)
for processing. This copy would take extra time, and could result in a significantly higher memory footprint when a file
includes very long lines. The potential time overhead from this can be seen in the Line Count benchmark below.

My implementation will also iterate twice over this line buffer depending on command line parameters: once for counting
the words, and a second time to count the characters in that line. It is, however, a bit more granular than the C version.
In the C version, if you want to only count words, it will also count characters as well. Mine will not, as these are two
separate code paths.

The parellelism is on a per-file basis. An individual file is processed serially, but multiple files can be processed
at the same time. This is done with the [Rayon](https://docs.rs/rayon/1.3.0/rayon/) library.

## Benchmarks

### Environment

The environment I tested this on is Ubuntu 19.04, running on VMWare Workstation 15. The system it was running on:

* Windows 10
* Intel i7 6700k (stock clock)
* 16Gb of RAM (8Gb available to the VM)
* Files were on a Crucial MX200 SSD on the VM's virtual drive.

### Results

**Default Settings**

This test counts the file length in bytes, and the word and line counts.

```
stuart@ubuntu:~$ wc a.txt 
  2136046  18873888 112638339 a.txt
stuart@ubuntu:~$ ./wc_r a.txt 
 2136046 18873888 112638339 a.txt

stuart@ubuntu:~$ hyperfine --warmup 5 'wc a.txt' './wc_r a.txt'
Benchmark #1: wc a.txt
  Time (mean ± σ):     527.3 ms ±   4.1 ms    [User: 509.0 ms, System: 17.1 ms]
  Range (min … max):   521.1 ms … 534.6 ms    10 runs
 
Benchmark #2: ./wc_r a.txt
  Time (mean ± σ):     392.7 ms ±   3.1 ms    [User: 376.1 ms, System: 16.3 ms]
  Range (min … max):   388.7 ms … 397.9 ms    10 runs
 
Summary
  './wc_r a.txt' ran
    1.34 ± 0.01 times faster than 'wc a.txt'
```

**Max Line Length/Char Count**

The results shown below only show the Max Line Length count, but in both programs these two options use the same code path.

Note that the timing is the same as the default settings for the C version.

```
stuart@ubuntu:~$ wc -L a.txt 
2541 a.txt
stuart@ubuntu:~$ ./wc_r -L a.txt 
 2541 a.txt


stuart@ubuntu:~$ hyperfine --warmup 5 'wc -L a.txt' './wc_r -L a.txt'
Benchmark #1: wc -L a.txt
  Time (mean ± σ):     530.4 ms ±   3.1 ms    [User: 513.8 ms, System: 15.6 ms]
  Range (min … max):   525.9 ms … 536.8 ms    10 runs
 
Benchmark #2: ./wc_r -L a.txt
  Time (mean ± σ):     162.5 ms ±   4.0 ms    [User: 146.9 ms, System: 15.0 ms]
  Range (min … max):   158.1 ms … 174.9 ms    18 runs
 
Summary
  './wc_r -L a.txt' ran
    3.26 ± 0.08 times faster than 'wc -L a.txt'
```

**Line Count**

These results demonstrate the time overhead of the second buffer in my implementation.

```
stuart@ubuntu:~$ wc -l a.txt 
2136046 a.txt
stuart@ubuntu:~$ ./wc_r -l a.txt 
 2136046 a.txt

stuart@ubuntu:~$ hyperfine --warmup 5 'wc -l a.txt' './wc_r -l a.txt'
Benchmark #1: wc -l a.txt
  Time (mean ± σ):      30.7 ms ±   0.8 ms    [User: 18.6 ms, System: 11.9 ms]
  Range (min … max):    29.8 ms …  33.7 ms    95 runs
 
Benchmark #2: ./wc_r -l a.txt
  Time (mean ± σ):      99.7 ms ±   1.5 ms    [User: 80.6 ms, System: 18.3 ms]
  Range (min … max):    97.8 ms … 104.5 ms    29 runs
 
Summary
  'wc -l a.txt' ran
    3.25 ± 0.10 times faster than './wc_r -l a.txt'
```

**Word Count**

This operation seems to be the most expensive. Note that the timing of both programs is the same as the default settings.

```
stuart@ubuntu:~$ wc -w a.txt 
18873888 a.txt
stuart@ubuntu:~$ ./wc_r -w a.txt 
 18873888 a.txt

stuart@ubuntu:~$ hyperfine --warmup 5 'wc -w a.txt' './wc_r -w a.txt'
Benchmark #1: wc -w a.txt
  Time (mean ± σ):     536.1 ms ±  12.6 ms    [User: 517.2 ms, System: 16.9 ms]
  Range (min … max):   524.7 ms … 567.1 ms    10 runs
 
Benchmark #2: ./wc_r -w a.txt
  Time (mean ± σ):     392.7 ms ±   3.1 ms    [User: 370.5 ms, System: 20.5 ms]
  Range (min … max):   388.9 ms … 397.8 ms    10 runs
 
Summary
  './wc_r -w a.txt' ran
    1.37 ± 0.03 times faster than 'wc -w a.txt'
```

**Word *and* Char counts**

This is the worst case input for my implementation. Having these two options enabled will result in the secondary
buffer being iterated over twice, which is reflected in the increased runtime.

```
stuart@ubuntu:~$ wc -mw a.txt 
 18873888 112294655 a.txt
stuart@ubuntu:~$ ./wc_r -mw a.txt 
 18873888 112294655 a.txt

stuart@ubuntu:~$ hyperfine --warmup 5 'wc -mw a.txt' './wc_r -mw a.txt'
Benchmark #1: wc -mw a.txt
  Time (mean ± σ):     528.3 ms ±   5.8 ms    [User: 509.8 ms, System: 16.9 ms]
  Range (min … max):   523.6 ms … 539.6 ms    10 runs
 
Benchmark #2: ./wc_r -mw a.txt
  Time (mean ± σ):     461.9 ms ±   4.8 ms    [User: 436.9 ms, System: 23.2 ms]
  Range (min … max):   456.7 ms … 473.1 ms    10 runs
 
Summary
  './wc_r -mw a.txt' ran
    1.14 ± 0.02 times faster than 'wc -mw a.txt'
```

**Multiple Files**

Just a quick test of handling multiple files. Note that the three files have identical contents.

```
stuart@ubuntu:~$ wc a.txt b.txt c.txt
  2136046  18873888 112638339 a.txt
  2136046  18873888 112638339 b.txt
  2136046  18873888 112638339 c.txt
  6408138  56621664 337915017 total
stuart@ubuntu:~$ ./wc_r a.txt b.txt c.txt
 2136046 18873888 112638339 b.txt
 2136046 18873888 112638339 c.txt
 2136046 18873888 112638339 a.txt
 6408138 56621664 337915017 total

stuart@ubuntu:~$ hyperfine --warmup 5 'wc a.txt b.txt c.txt' './wc_r a.txt b.txt c.txt'
Benchmark #1: wc a.txt b.txt c.txt
  Time (mean ± σ):      1.570 s ±  0.008 s    [User: 1.525 s, System: 0.044 s]
  Range (min … max):    1.555 s …  1.581 s    10 runs
 
Benchmark #2: ./wc_r a.txt b.txt c.txt
  Time (mean ± σ):     456.3 ms ±  21.4 ms    [User: 1.245 s, System: 0.060 s]
  Range (min … max):   434.4 ms … 508.6 ms    10 runs
 
Summary
  './wc_r a.txt b.txt c.txt' ran
    3.44 ± 0.16 times faster than 'wc a.txt b.txt c.txt'
```