# Pipe viewer reimplementation
`pv` is a Unix pipe monitoring application. (And this is copy of the much older original)

You can use it in places where a progressbar, or at least a flow rate meter,
would be handy. Some handy examples:

```sh
# Is it still transferring or did something freeze?
docker save excelsior | pv | ssh me@devbox.company.com "docker load"
```

```sh
# Why doesn't gzip have a progressbar already?
pv gigantic-file | gunzip | gawk '/size/ { x += $4 } END {print x}'
```

## Feature Comparison with Standard pv

This Rust implementation covers the core functionality of the original `pv` utility but is missing several advanced features. Here's a comparison:

| Feature | Standard pv | This Implementation | Status |
|---------|-------------|-------------------|--------|
| **Core Display** |
| Progress bar (`-p`) | ✅ | ✅ | ✅ Implemented |
| Timer (`-t`) | ✅ | ✅ | ✅ Implemented |
| ETA (`-e`) | ✅ | ✅ | ✅ Implemented |
| Finish ETA (`-I`) | ✅ | ✅ | ✅ Implemented |
| Rate counter (`-r`) | ✅ | ✅ | ✅ Implemented |
| Average rate (`-a`) | ✅ | ✅ | ✅ Implemented |
| Byte counter (`-b`) | ✅ | ✅ | ✅ Implemented |
| Line mode (`-l`) | ✅ | ✅ | ✅ Implemented |
| Null termination (`-0`) | ✅ | ✅ | ✅ Implemented |
| Size specification (`-s`) | ✅ | ✅ | ✅ Implemented |
| Width control (`-w`) | ✅ | ✅ | ✅ Implemented |
| Name prefix (`-N`) | ✅ | ✅ | ✅ Implemented |
| Update interval (`-i`) | ✅ | ✅ | ✅ Implemented |
| Skip input errors (`-E`) | ✅ | ✅ | ✅ Implemented |
| **Missing Core Features** |
| Buffer percentage (`-T`) | ✅ | ❌ | 🔴 **Not Implemented** |
| Last written bytes (`-A`) | ✅ | ❌ | 🔴 **Not Implemented** |
| Custom format (`-F`) | ✅ | ❌ | 🔴 **Not Implemented** |
| Numeric output (`-n`) | ✅ | ❌ | 🔴 **Not Implemented** |
| Quiet mode (`-q`) | ✅ | ❌ | 🔴 **Not Implemented** |
| **Missing Display Options** |
| Bits instead of bytes (`-8`) | ✅ | ❌ | 🔴 **Not Implemented** |
| SI units (`-k`) | ✅ | ❌ | 🔴 **Not Implemented** |
| Wait for first byte (`-W`) | ✅ | ❌ | 🔴 **Not Implemented** |
| Delay start (`-D`) | ✅ | ❌ | 🔴 **Not Implemented** |
| Gauge mode (`-g`) | ✅ | ❌ | 🔴 **Not Implemented** |
| Average rate window (`-m`) | ✅ | ❌ | 🔴 **Not Implemented** |
| Bar style (`-u`) | ✅ | ❌ | 🔴 **Not Implemented** |
| Extra display (`-x`) | ✅ | ❌ | 🔴 **Not Implemented** |
| Transfer stats (`-v`) | ✅ | ❌ | 🔴 **Not Implemented** |
| Force output (`-f`) | ✅ | ❌ | 🔴 **Not Implemented** |
| Cursor positioning (`-c`) | ✅ | ❌ | 🔴 **Not Implemented** |
| **Missing Data Transfer Features** |
| Output to file (`-o`) | ✅ | ❌ | 🔴 **Not Implemented** |
| Rate limiting (`-L`) | ✅ | ❌ | 🔴 **Not Implemented** |
| Buffer size control (`-B`) | ✅ | ❌ | 🔴 **Not Implemented** |
| No splice (`-C`) | ✅ | ❌ | 🔴 **Not Implemented** |
| Skip output errors (`-O`) | ✅ | ✅ | ✅ Implemented |
| Error skip blocks (`-Z`) | ✅ | ❌ | 🔴 **Not Implemented** |
| Stop at size (`-S`) | ✅ | ❌ | 🔴 **Not Implemented** |
| Sync writes (`-Y`) | ✅ | ❌ | 🔴 **Not Implemented** |
| Direct I/O (`-K`) | ✅ | ❌ | 🔴 **Not Implemented** |
| Discard output (`-X`) | ✅ | ❌ | 🔴 **Not Implemented** |
| Store and forward (`-U`) | ✅ | ❌ | 🔴 **Not Implemented** |
| **Missing Advanced Features** |
| Watch file descriptor (`-d`) | ✅ | ❌ | 🔴 **Not Implemented** |
| Remote control (`-R`) | ✅ | ❌ | 🔴 **Not Implemented** |
| PID file (`-P`) | ✅ | ❌ | 🔴 **Not Implemented** |

### Implementation Priority

**High Priority (Core Missing Features):**
- [ ] Custom format strings (`-F`) - Essential for scripting and integration
- [ ] Numeric output (`-n`) - Important for automation
- [ ] Rate limiting (`-L`) - Common use case for bandwidth control  
- [ ] Output to file (`-o`) - Basic I/O redirection
- [ ] Force output (`-f`) - Important for non-terminal usage
- [ ] Quiet mode (`-q`) - Essential for silent operation

**Medium Priority (Enhanced Display):**
- [ ] Buffer percentage (`-T`) - Useful debugging feature
- [ ] Transfer statistics (`-v`) - Nice summary feature
- [ ] Gauge mode (`-g`) - Alternative progress display
- [ ] SI units (`-k`) - Standards compliance
- [ ] Bits display (`-8`) - Network monitoring use case

**Lower Priority (Advanced Features):**
- [ ] Watch file descriptor (`-d`) - Advanced monitoring feature
- [ ] Remote control (`-R`) - Advanced process control
- [ ] Store and forward (`-U`) - Specialized use case
- [ ] Direct I/O (`-K`) - Performance optimization
- [ ] Cursor positioning (`-c`) - Terminal control feature

### Summary

The current implementation covers approximately **45%** of the standard `pv` features (21 out of 46 options). It successfully implements the core progress monitoring functionality but lacks many advanced features that make the original `pv` versatile for different use cases.