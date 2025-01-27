# TimerResBenchmark

<p align="center"><b>A Rust-based tool for benchmarking system timer resolution to achieve precise sleep intervals, optimizing performance and consistency for high-performance tasks like gaming, especially in low-latency scenarios. It automatically detects HPET status and identifies the optimal timer resolution for your system.</b></p>

![image](https://github.com/user-attachments/assets/72b39d18-94a8-4312-b7ac-d86f773520ce)


---

## Features
- Checks if HPET is enabled or disabled during benchmark setup.
 ![image](https://github.com/user-attachments/assets/6ce5c4d0-98b1-420c-87d5-e5a1f68ba81d)
- **Customizable Benchmark Parameters**
   - Modify the start value, end value, increment value and sample value through the program/config file.
     ![timer_res_benchmark_xUHblQxThy](https://github.com/user-attachments/assets/a289bd81-ee0f-4c14-af3e-fe21c590b927)

- **Automatically detects the optimal timer resolution for your system** based on the lowest sleep delta and standard deviation.
  ![image](https://github.com/user-attachments/assets/c6cac925-f872-4ae7-b355-e203ce1996af)

- **Rewritten in Rust** for better performance and less overhead.

---

## Installation and Usage

1. **Disable HPET** and set up an **Idle-disabled power plan** (follow the [Troubleshooting](https://github.com/SwiftyPop/TimerResBenchmark/tree/main?tab=readme-ov-file#troubleshooting) guide for help).
2. Download the latest release from [GitHub Releases](https://github.com/SwiftyPop/TimerResBenchmark/releases).
3. Extract the `.7z` archive.
4. Run `timer_res_benchmark.exe` as an administrator.
   - You can adjust the benchmark parameters directly in the program or modify them manually in the 'appsettings.json' file(default value).
5. After the benchmark completes, it will automatically detect the optimal timer resolution for your system. The results and additional details will be saved in the 'results.txt' file.

---

### Step 2: Set the Optimal Timer Resolution
1. Create a shortcut for `SetTimerResolution.exe`.
2. Place the shortcut in your `shell:startup` folder with the following target:
```
C:\PATH\TO\SetTimerResolution.exe --no-console --resolution 5000
```
- Replace `5000` with your optimal resolution (e.g., `5000` for 0.5ms).
3. Restart your PC and verify the settings using `MeasureSleep.exe`.

---
### [Optional] Visualize the Results
1. Visit [Plotly Chart Studio](https://chart-studio.plotly.com/create/#/).
2. Click **"Import"** at the top right and upload the `results.txt` file.
3. Add a trace and configure the settings as shown below:
   ![Plotly Configuration](https://github.com/SwiftyPop/TimerResBenchmark/assets/90952326/9f08eb09-7e1a-41f5-819e-10bd41444cd9)
4. Look for the lowest `Sleep(1) Delta` on the y-axis. This represents the most precise and consistent 1ms sleep delays.
   - Example: If the lowest delta is at 0.5024ms, this is your optimal timer resolution.

---

## Troubleshooting

### Sleep Delays Are Spiking (>1ms)
1. **Disable HPET**:
- Open Command Prompt as administrator and run:
  ```bash
  bcdedit /deletevalue useplatformclock
  bcdedit /set disabledynamictick yes
  ```
- On Windows Server 2022+ and Windows 11+, apply this registry change:
  ```plaintext
  [HKEY_LOCAL_MACHINE\SYSTEM\CurrentControlSet\Control\Session Manager\kernel]
  "GlobalTimerResolutionRequests"=dword:00000001
  ```
- You can now verify HPET status via this program:
![image](https://github.com/user-attachments/assets/6ce5c4d0-98b1-420c-87d5-e5a1f68ba81d)


2. **Use an Idle-Disabled Power Plan**:
- Download the [Muren power plan](https://www.mediafire.com/file/39yxlxpbkyjg3qa/Muren.pow/file).
- Import the plan using Command Prompt:
  ```bash
  powercfg -import C:\PATH\TO\MUREN.POW
  ```
- Set your power plan to **Muren**.

---

## Why Use TimerResBenchmark?
- Disables HPET and uses more stable timers (e.g., TSC at 3.32MHz) for better frame rate consistency and lower latency.
- Unlike the original PowerShell-based benchmark, this tool is now a native executable written in Rust, making it faster and easier to use.
- This tool was initially rewritten from C# to Rust as part of a learning project. Contributions and feedback are always welcome!

---

## License
This project is licensed under the **MIT License**. See the [LICENSE](https://github.com/SwiftyPop/TimerResBenchmark/blob/master/LICENSE) file for details.
