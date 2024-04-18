# TimerResBenchmark
<p align="center"><b> TimerResBenchmark is a executable benchmark to micro-adjusting timer resolution for higher precision. This tool identifies the most accurate timer resolution, striving to achieve the shortest possible delay intervals close to 1 millisecond. Achieving this level of precision is beneficial for the overall performance of the system. </b></p> 

* The tool also include TimerResolution & MeasureSleep from [Amittxv's TimerRes](https://github.com/amitxv/TimerResolution/tree/main).

![gifgit](https://github.com/SwiftyPop/TimerResBenchmark/assets/90952326/f03feefd-4bf0-4c05-893e-14570f785d3a)

## Installation Guide 

### ⚙️ Step 1: Downloading and Running the Tool
- Before we start, ensure to [disable HPET & use Idle disabled power plan](https://github.com/SwiftyPop/TimerResBenchmark/edit/master/README.md#troubleshooting).
- Grab the latest version at [GitHub Releases](https://github.com/SwiftyPop/TimerResBenchmark/releases).
- Extract the `.7z` archive.
- Run `TimerBenchmark.exe` as an administrator(Adjust settings in `appsettings.json` as needed).
- It will generate a `result.txt` file in the same directory upon completion.

### 📊 Step 2: Visualize the Results & Find your Optimal Timer Resolution

To visualize the results:

1. Visit [Plotly Chart Studio](https://chart-studio.plotly.com/create/#/).
2. Click "Import" at the top right and upload the `result.txt` file.
3. Click "+ Trace" at the top left and adjust the settings as shown in the image below:
 ![vivaldi_9APYujDnIj](https://github.com/SwiftyPop/TimerResBenchmark/assets/90952326/9f08eb09-7e1a-41f5-819e-10bd41444cd9)

  - The graph should work, and this time we finally can find that sweet timer resolution.
   ![NWsnWsn3Ax](https://github.com/SwiftyPop/TimerResBenchmark/assets/90952326/24a33f65-2edd-464e-b49d-43ed1497d0b1)
  - Look for the lowest Sleep(1) Delta on the y-axis. This represents the most precise and constant 1ms Sleep delays. Remember, this value varies greatly between PC specs. Based on my lowest lowest Sleep(1) Delta, the optimal resolution for me is 0.5024ms

### 🚀 Step 3: Set Up Timer Resolution
1. Go back to the directory where `TimerBenchmark.exe` is located.
2. Create a shortcut for `SetTimerResolution.exe`.
3. Place the shortcut in `shell:startup` with the following target:
```
C:\PATH\TO\SetTimerResolution.exe --no-console --resolution 5000
```
- Replace `5000` with your optimal timer resolution.
- Restart your PC & verify everything is working correctly with `MeasureSleep.exe`.

  
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
3. Change your power plan setting to Muren power plan.

## Why & What the purpose of this?
- The purpose of this is to disable HPET and utilize more stable and consistent timers, such as TSC at 3.32MHz, which can improve gaming performance, particularly in terms of frame rate consistency and latency. The benchmark allows you to find the perfect adjustment of timer resolution based on benchmark results, which can further enhance these benefits.
- The original [benchmark](https://github.com/amitxv/TimerResolution/blob/main/micro-adjust-benchmark.ps1) required execution via Command Prompt or PowerShell, which may not be user-friendly for all users. Therefore, TimerResBenchmark provides an executable version that is easy to run and written in C# for efficiency.
- This project was created as a learning endeavor to explore the C# language further. Contributions and insights are welcome to help improve the tool and the author's understanding of C#. :)





