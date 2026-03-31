export def info [msg: string] {
    print $"[(ansi blue)INFO(ansi reset)] ($msg)"
}

export def warn [msg: string] {
    print $"[(ansi yellow)WARN(ansi reset)] ($msg)"
}

export def ok [msg: string] {
    print $"[(ansi green)OK(ansi reset)] ($msg)"
}

export def err [msg: string] {
    print $"[(ansi red)ERROR(ansi reset)] ($msg)"
}
