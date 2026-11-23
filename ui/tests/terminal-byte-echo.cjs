process.stdin.setRawMode(true)
process.stdout.write('\x1b[?2004hRAW READY\r\n')
process.stdin.on('data', (bytes) => {
  process.stdout.write(`INPUT ${bytes.toString('hex')}\r\n`)
  if (bytes.includes(4)) process.exit(0)
})
