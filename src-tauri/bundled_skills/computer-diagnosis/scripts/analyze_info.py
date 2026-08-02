import argparse
import json

def analyze_system_data(file_path):
    with open(file_path, 'r', encoding='utf-8') as f:
        data = json.load(f)

    issues = []
    
    # 1. CPU Analysis
    cpu_usage_str = data.get("CPU Info", {}).get("Total CPU Usage", "0%")
    cpu_usage = float(cpu_usage_str.strip('%'))
    if cpu_usage > 85:
        issues.append({
            "category": "CPU",
            "severity": "High",
            "issue": f"CPU usage is high at {cpu_usage_str}.",
            "recommendation": "Check for background processes consuming high CPU using Task Manager or Activity Monitor. Consider closing unnecessary applications."
        })

    # 2. Memory Analysis
    mem_percent_str = data.get("Memory Info", {}).get("Percentage Virtual Memory", "0%")
    mem_percent = float(mem_percent_str.strip('%'))
    if mem_percent > 85:
        issues.append({
            "category": "Memory",
            "severity": "High",
            "issue": f"Memory usage is high at {mem_percent_str}.",
            "recommendation": "System is running low on RAM. Close memory-intensive applications. Consider adding more RAM if this is a frequent issue."
        })

    # 3. Disk Analysis
    disks = data.get("Disk Info", [])
    for disk in disks:
        disk_percent_str = disk.get("Percentage", "0%")
        disk_percent = float(disk_percent_str.strip('%'))
        if disk_percent > 90:
            issues.append({
                "category": "Disk",
                "severity": "Critical",
                "issue": f"Disk '{disk.get('Mountpoint')}' is almost full at {disk_percent_str}.",
                "recommendation": f"Free up space on {disk.get('Mountpoint')} immediately. Use built-in disk cleanup tools or delete unnecessary large files."
            })
        elif disk_percent > 80:
            issues.append({
                "category": "Disk",
                "severity": "Medium",
                "issue": f"Disk '{disk.get('Mountpoint')}' is getting full at {disk_percent_str}.",
                "recommendation": f"Consider freeing up some space on {disk.get('Mountpoint')} to ensure smooth system operation."
            })

    report = {
        "status": "Healthy" if not issues else "Needs Attention",
        "issues_found": len(issues),
        "issues": issues
    }
    
    return report

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Analyze system metrics and generate a report.")
    parser.add_argument("input_file", help="Path to the JSON file containing system data")
    args = parser.parse_args()
    
    analysis_result = analyze_system_data(args.input_file)
    print(json.dumps(analysis_result, indent=4))