export def info [msg: string] {
    print --stderr $"[(ansi blue)INFO(ansi reset)] ($msg)"
}

export def warning [msg: string] {
    print --stderr $"[(ansi yellow)WARN(ansi reset)] ($msg)"
}

export def error [msg: string] {
    print --stderr $"[(ansi red)ERROR(ansi reset)] ($msg)"
}

export def ok [msg: string] {
    print --stderr $"[(ansi green)OK(ansi reset)] ($msg)"
}
