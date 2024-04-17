using System.Diagnostics;
using System.Diagnostics.CodeAnalysis;
using System.Security.Principal;
using Microsoft.Extensions.Configuration;

namespace TimerBenchmark;

internal abstract class TimerBenchmark
{
    private static bool? _isAdmin;

    private static bool IsAdmin()
    {
        if (_isAdmin == null)
        {
            _isAdmin = new WindowsPrincipal(WindowsIdentity.GetCurrent()).IsInRole(WindowsBuiltInRole.Administrator);
        }

        return _isAdmin.Value;
    }

    [RequiresDynamicCode("Calls Microsoft.Extensions.Configuration.ConfigurationBinder.Get<T>()")]
    [RequiresUnreferencedCode("Calls Microsoft.Extensions.Configuration.ConfigurationBinder.Get<T>()")]
    private static async Task Main()
    {
        IConfiguration config = new ConfigurationBuilder()
            .AddJsonFile("appsettings.json", optional: false, reloadOnChange: true)
            .Build();

        BenchmarkingParameters? parameters = config.GetSection("BenchmarkingParameters").Get<BenchmarkingParameters>();

        if (parameters is null)
        {
            Console.WriteLine("Error: Unable to read configuration parameters.");
            return;
        }

        if (!IsAdmin())
        {
            await Console.Error.WriteLineAsync("error: administrator privileges required");
            Environment.Exit(1);
        }

        decimal iterations =
            (decimal)(parameters.EndValue - parameters.StartValue) / (decimal)parameters.IncrementValue;
        decimal totalMinutes = iterations * parameters.SampleValue * 2 / 60000m; // Assuming Sleep(1) = ~2ms

        Console.WriteLine($"Approximate worst-case estimated time for completion: {Math.Round(totalMinutes, 2)} mins");
        Console.WriteLine("Worst-case is determined by assuming Sleep(1) = ~2ms with 1ms Timer Resolution");
        Console.WriteLine(
            $"Start: {parameters.StartValue}, End: {parameters.EndValue}, Increment: {parameters.IncrementValue}, Samples: {parameters.SampleValue}");

        KillProcess("SetTimerResolution");
        string currentDirectory = Environment.CurrentDirectory;

        string[] dependencies = new[] { "SetTimerResolution.exe", "MeasureSleep.exe" };

        bool hasMissingDependencies = false;
        object missingDependenciesLock = new object();

        Parallel.ForEach(dependencies, dependency =>
        {
            string fullPath = Path.Combine(currentDirectory, dependency);
            if (!File.Exists(fullPath))
            {
                lock (missingDependenciesLock)
                {
                    if (!hasMissingDependencies)
                    {
                        Console.WriteLine($"Error: {dependency} does not exist in the current directory");
                        hasMissingDependencies = true;
                    }
                }
            }
        });

        if (hasMissingDependencies)
        {
            return;
        }

        string content = "RequestedResolutionMs,DeltaMs,STDEV";
        await File.WriteAllTextAsync("results.txt", content);

        for (double i = parameters.StartValue; i <= parameters.EndValue; i += parameters.IncrementValue)
        {
            double formattedValue = Math.Round(i, 4, MidpointRounding.AwayFromZero);
            Console.WriteLine($"info: benchmarking {formattedValue}");

            int resolution = (int)(formattedValue * 1E4);
            await Task.Run(() =>
            {
                Process.Start(Path.Combine(currentDirectory, "SetTimerResolution.exe"),
                    $"--resolution {resolution} --no-console");
            });

            // Delay after setting resolution
            await Task.Delay(1);

            ProcessStartInfo startInfo = new ProcessStartInfo
            {
                FileName = Path.Combine(currentDirectory, "MeasureSleep.exe"),
                Arguments = $"--samples {parameters.SampleValue}",
                UseShellExecute = false,
                RedirectStandardOutput = true
            };

            Process? process = Process.Start(startInfo);
            string output = await process?.StandardOutput.ReadToEndAsync()!;
            await process.WaitForExitAsync();

            string[] outputLines = output.Split(new[] { Environment.NewLine }, StringSplitOptions.RemoveEmptyEntries);
            (double avg, double stdev) = (0, 0);

            foreach (var line in outputLines)
            {
                if (line.StartsWith("Avg: ") && double.TryParse(line.AsSpan(5), out var parsedAvg))
                {
                    avg = parsedAvg;
                }
                else if (line.StartsWith("STDEV: ") && double.TryParse(line.AsSpan(7), out var parsedStdev))
                {
                    stdev = parsedStdev;
                }
            }

            string resultLine = $"{formattedValue}, {Math.Round(avg, 4)}, {stdev}{Environment.NewLine}";
            await File.AppendAllTextAsync("results.txt", resultLine);

            KillProcess("SetTimerResolution");
        }

        Console.WriteLine("info: results saved in results.txt");
    }

    private class BenchmarkingParameters
    {
        public double StartValue { get; init; }
        public double IncrementValue { get; init; }
        public double EndValue { get; init; }
        public int SampleValue { get; init; }
    }

    private static void KillProcess(string processName)
    {
        foreach (var process in Process.GetProcessesByName(processName))
        {
            process.Kill();
        }
    }
}