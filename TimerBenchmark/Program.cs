using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Security.Principal;
using System.Text;
using Microsoft.Extensions.Configuration;

class Program
{
    static bool IsAdmin()
    {
        WindowsPrincipal principal = new WindowsPrincipal(WindowsIdentity.GetCurrent());
        return principal.IsInRole(WindowsBuiltInRole.Administrator);
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

        int iterations = (int)Math.Round((parameters.EndValue - parameters.StartValue) / parameters.IncrementValue);
        double totalMs = iterations * parameters.SampleValue;
        Console.WriteLine($"Approximate worst-case estimated time for completion: {Math.Round(totalMs / 6E4, 2)}mins");
        Console.WriteLine("Worst-case is determined by assuming Sleep(1) = ~2ms with 1ms Timer Resolution");
        Console.WriteLine($"Start: {parameters.StartValue}, End: {parameters.EndValue}, Increment: {parameters.IncrementValue}, Samples: {parameters.SampleValue}");

        KillProcess("SetTimerResolution");
        string currentDir = Environment.CurrentDirectory;

        foreach (string dependency in new[] { "SetTimerResolution.exe", "MeasureSleep.exe" })
        {
            if (!File.Exists(Path.Combine(currentDir, dependency)))
            {
                Console.WriteLine($"error: {dependency} not exists in current directory");
                return;
            }
        }

        File.WriteAllText("results.txt", "RequestedResolutionMs,DeltaMs,STDEV");

        for (double i = parameters.StartValue; i <= parameters.EndValue; i += parameters.IncrementValue)
        {
            double formattedValue = Math.Round(i, 4);
            Console.WriteLine($"info: benchmarking {formattedValue}");

            int resolution = (int)(formattedValue * 1E4);
            Process.Start(Path.Combine(currentDir, "SetTimerResolution.exe"), $"--resolution {resolution} --no-console");

            // unexpected results if there isn't a small delay after setting the resolution
            System.Threading.Thread.Sleep(1);

            ProcessStartInfo startInfo = new ProcessStartInfo
            {
                FileName = Path.Combine(currentDir, "MeasureSleep.exe"),
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
                    avg = double.Parse(line.Substring(5));
                else if (line.StartsWith("STDEV: "))
                    stdev = double.Parse(line.Substring(7));
            }

            File.AppendAllText("results.txt", $"{formattedValue}, {Math.Round(avg, 4)}, {stdev}{Environment.NewLine}");
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
        foreach (Process process in Process.GetProcessesByName(processName))
        {
            process.Kill();
        }
    }
}