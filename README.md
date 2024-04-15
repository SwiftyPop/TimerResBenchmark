# TimerResBenchmark

TimerResBenchmark is a executable benchmark written in C# to micro-adjusting timer resolution for higher precision. The tool aims to find the best timer resolution that are closest possible sleep delays to 1ms, which is considered optimal for system performance. It is recommended to use a power plan with idle disabled to achieve consistent 1ms sleep delays and variance in the delta. The tool also include TimerResolution & MeasureSleep from [Amittxv's TimerRes](https://github.com/amitxv/TimerResolution/tree/main), and users can configure settings via `appsettings.json`. For a detailed explanation, visit the [Amittxv's TimerResolution](https://github.com/amitxv/TimerResolution/tree/main).


![gifgit](https://github.com/SwiftyPop/TimerResBenchmark/assets/90952326/f03feefd-4bf0-4c05-893e-14570f785d3a)

## Installation Guide 

### Downloading and Running the Tool
- Obtain the latest version from the [GitHub Releases](https://github.com/SwiftyPop/TimerResBenchmark/releases).
- Extract the `.7z` archive.
- Run `TimerBenchmark.exe` as an administrator.
- Adjust settings in `appsettings.json` as needed.
- It will generate `result.txt` in the same file directory and you can visualize the result at https://chart-studio.plotly.com/create
  ![amitxv's graph](https://github.com/SwiftyPop/TimerResBenchmark/assets/90952326/dadb2abb-0597-442c-998f-8d7574c0a56b)


## Troubleshooting:
### My Sleep Delays Are Spiking and High >1ms and the Delta Keeps Spiking
##### Make Sure to disable HPET first by
1. Open CMD with administrator privileges and execute the following commands:
  ~~~
  bcdedit /deletevalue useplatformclock
  bcdedit /set disabledynamictick yes
  ~~~

2. On Windows Server 2022+ and Windows 11+, apply the following registry change:
  ~~~
  [HKEY_LOCAL_MACHINE\SYSTEM\CurrentControlSet\Control\Session Manager\kernel]
  "GlobalTimerResolutionRequests"=dword:00000001
  ~~~

3. Verify the HPET status using [TimerBench](https://www.overclockers.at/articles/the-hpet-bug-what-it-is-and-what-it-isnt)

#### Use Idle disabled power plans
1. You can either configure it manually or download the [power plan](https://www.mediafire.com/file/39yxlxpbkyjg3qa/Muren.pow/file)
2. Open CMD & type this command:
~~~
powercfg -import C:\PATH\TO\MUREN.POW
~~~

## Why & What the purpose of this?
- The purpose of this is to disable HPET and utilize more stable and consistent timers, such as TSC at 3.32MHz, which can improve gaming performance, particularly in terms of frame rate consistency and latency. The benchmark allows you to find the perfect adjustment of timer resolution based on benchmark results, which can further enhance these benefits.
- The original [benchmark](https://github.com/amitxv/TimerResolution/blob/main/micro-adjust-benchmark.ps1) required execution via Command Prompt or PowerShell, which may not be user-friendly for all users. Therefore, TimerResBenchmark provides an executable version that is easy to run and written in C# for efficiency.
- This project was created as a learning endeavor to explore the C# language further. Contributions and insights are welcome to help improve the tool and the author's understanding of C#. :)





