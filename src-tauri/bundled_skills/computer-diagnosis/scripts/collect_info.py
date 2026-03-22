import platform
import psutil
import socket
import os
import sys

def get_size(bytes, suffix="B"):
    """
    Scale bytes to its proper format
    e.g:
        1253656 => '1.20MB'
        1253656678 => '1.17GB'
    """
    factor = 1024
    for unit in ["", "K", "M", "G", "T", "P"]:
        if bytes < factor:
            return f"{bytes:.2f}{unit}{suffix}"
        bytes /= factor

def get_system_info():
    uname = platform.uname()
    return {
        "System": uname.system,
        "Node Name": uname.node,
        "Release": uname.release,
        "Version": uname.version,
        "Machine": uname.machine,
        "Processor": uname.processor
    }

def get_cpu_info():
    cpufreq = psutil.cpu_freq()
    return {
        "Physical cores": psutil.cpu_count(logical=False),
        "Total cores": psutil.cpu_count(logical=True),
        "Max Frequency": f"{cpufreq.max:.2f}Mhz",
        "Min Frequency": f"{cpufreq.min:.2f}Mhz",
        "Current Frequency": f"{cpufreq.current:.2f}Mhz",
        "CPU Usage Per Core": [f"{x}%" for x in psutil.cpu_percent(percpu=True, interval=1)],
        "Total CPU Usage": f"{psutil.cpu_percent()}%"
    }

def get_memory_info():
    svmem = psutil.virtual_memory()
    swap = psutil.swap_memory()
    return {
        "Total Virtual Memory": get_size(svmem.total),
        "Available Virtual Memory": get_size(svmem.available),
        "Used Virtual Memory": get_size(svmem.used),
        "Percentage Virtual Memory": f"{svmem.percent}%",
        "Total Swap Memory": get_size(swap.total),
        "Free Swap Memory": get_size(swap.free),
        "Used Swap Memory": get_size(swap.used),
        "Percentage Swap Memory": f"{swap.percent}%"
    }

def get_disk_info():
    partitions = psutil.disk_partitions()
    disk_data = []
    for partition in partitions:
        try:
            partition_usage = psutil.disk_usage(partition.mountpoint)
            disk_data.append({
                "Device": partition.device,
                "Mountpoint": partition.mountpoint,
                "File system type": partition.fstype,
                "Total Size": get_size(partition_usage.total),
                "Used": get_size(partition_usage.used),
                "Free": get_size(partition_usage.free),
                "Percentage": f"{partition_usage.percent}%"
            })
        except PermissionError:
            continue
    return disk_data

def get_network_info():
    if_addrs = psutil.net_if_addrs()
    net_io = psutil.net_io_counters()
    network_data = {"Interfaces": {}}
    for interface_name, interface_addresses in if_addrs.items():
        network_data["Interfaces"][interface_name] = []
        for address in interface_addresses:
            if str(address.family) == 'AddressFamily.AF_INET':
                network_data["Interfaces"][interface_name].append({
                    "Type": "IPv4",
                    "IP Address": address.address,
                    "Netmask": address.netmask,
                    "Broadcast IP": address.broadcast
                })
            elif str(address.family) == 'AddressFamily.AF_PACKET':
                network_data["Interfaces"][interface_name].append({
                    "Type": "MAC",
                    "MAC Address": address.address,
                    "Netmask": address.netmask,
                    "Broadcast MAC": address.broadcast
                })
    network_data["IO"] = {
        "Total Bytes Sent": get_size(net_io.bytes_sent),
        "Total Bytes Received": get_size(net_io.bytes_recv)
    }
    return network_data

if __name__ == "__main__":
    print("Gathering System Information...\n")
    import json
    
    report = {
        "System Information": get_system_info(),
        "CPU Info": get_cpu_info(),
        "Memory Info": get_memory_info(),
        "Disk Info": get_disk_info(),
        "Network Info": get_network_info()
    }
    
    print(json.dumps(report, indent=4))