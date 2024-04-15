using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Security.Principal;
using System.Text;
using Microsoft.Extensions.Configuration;
using System.Threading.Tasks;

class Program
{
    static bool IsAdmin()
    {
        return new WindowsPrincipal(WindowsIdentity.GetCurrent()).IsInRole(WindowsBuiltInRole.Administrator);
    }

    static void Main(string[] args)
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
            Console.WriteLine("error: administrator privileges required");
            return;
        }

        int iterations = (int)(((double)parameters.EndValue - parameters.StartValue) / parameters.IncrementValue);
        double totalMinutes = (double)iterations * parameters.SampleValue / 60000;

        string message = $"Approximate worst-case estimated time for completion: {Math.Round(totalMinutes, 2)}mins";
        string details = $"Start: {parameters.StartValue}, End: {parameters.EndValue}, Increment: {parameters.IncrementValue}, Samples: {parameters.SampleValue}";

        Console.WriteLine(message);
        Console.WriteLine("Worst-case is determined by assuming Sleep(1) = ~2ms with 1ms Timer Resolution");
        Console.WriteLine(details);

        KillProcess("SetTimerResolution");
        string currentDirectory = Environment.CurrentDirectory;

        string[] dependencies = { "SetTimerResolution.exe", "MeasureSleep.exe" };
        List<string> missingDependencies = new List<string>();

        Parallel.ForEach(dependencies, dependency =>
        {
            string fullPath = Path.Combine(currentDirectory, dependency);
            if (!File.Exists(fullPath))
            {
                missingDependencies.Add(dependency);
            }
        });

        if (missingDependencies.Count > 0)
        {
            foreach (var missingDependency in missingDependencies)
            {
                Console.WriteLine($"Error: {missingDependency} does not exist in the current directory");
            }
            return;
        }

        using (StreamWriter sw = new StreamWriter("results.txt"))
        {
            sw.WriteLine("RequestedResolutionMs,DeltaMs,STDEV");
        }

        for (double i = parameters.StartValue; i <= parameters.EndValue; i += parameters.IncrementValue)
        {
            double formattedValue = Math.Round(i, 4);
            Console.WriteLine($"info: benchmarking {formattedValue}");

            int resolution = (int)(formattedValue * 1E4);
            Task.Run(() =>
            {
                Process.Start(Path.Combine(currentDirectory, "SetTimerResolution.exe"), $"--resolution {resolution} --no-console");
            }).Wait();

            // unexpected results if there isn't a small delay after setting the resolution
            Task.Delay(1).Wait();

            ProcessStartInfo startInfo = new ProcessStartInfo
            {
                FileName = Path.Combine(currentDirectory, "MeasureSleep.exe"),
                Arguments = $"--samples {parameters.SampleValue}",
                UseShellExecute = false,
                RedirectStandardOutput = true
            };

            Process process = Process.Start(startInfo);
            string output = process?.StandardOutput.ReadToEndAsync().Result;
            process?.WaitForExit();

            string[] outputLines = output.Split(new[] { Environment.NewLine }, StringSplitOptions.RemoveEmptyEntries);
            double avg = 0, stdev = 0;

            foreach (string line in outputLines)
            {
                if (line.StartsWith("Avg: "))
                {
                    if (double.TryParse(line.Substring(5), out double parsedAvg))
                    {
                        avg = parsedAvg;
                    }
                }
                else if (line.StartsWith("STDEV: "))
                {
                    if (double.TryParse(line.Substring(7), out double parsedStdev))
                    {
                        stdev = parsedStdev;
                    }
                }
            }

            string resultLine = $"{formattedValue}, {Math.Round(avg, 4)}, {stdev}{Environment.NewLine}";
            File.AppendAllText("results.txt", resultLine);

            KillProcess("SetTimerResolution");
        }

        Console.WriteLine("info: results saved in results.txt");
    }

    private class BenchmarkingParameters
    {
        public double StartValue { get; set; }
        public double IncrementValue { get; set; }
        public double EndValue { get; set; }
        public int SampleValue { get; set; }
    }

    static void KillProcess(string processName)
    {
        Process.GetProcessesByName(processName)
               .ToList()
               .ForEach(p => p.Kill());
    }

}