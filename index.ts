import readline from 'node:readline'

const rl = readline.createInterface({
    input: process.stdin,
    output: process.stdout,
    terminal: false,
});

rl.on('line', (line: string) => {
    process.stdout.write(`${line}\n`);
})

console.error("Sidecar started");