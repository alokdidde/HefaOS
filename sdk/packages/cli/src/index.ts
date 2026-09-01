#!/usr/bin/env node
/**
 * @hefaos/cli - Development tools for Hefaos robotics projects
 *
 * @packageDocumentation
 */

import { Command } from 'commander';

const program = new Command();

program
  .name('hefaos')
  .description('Hefaos CLI - Development tools for robotics')
  .version('0.1.0');

// ─────────────────────────────────────────────────────────────────────────────
// Commands
// ─────────────────────────────────────────────────────────────────────────────

program
  .command('init [name]')
  .description('Initialize a new Hefaos robot project')
  .option('-t, --template <template>', 'Project template', 'basic')
  .action(async (name, options) => {
    const { init } = await import('./commands/init');
    await init(name, options);
  });

program
  .command('build [path]')
  .description('Compile robot definition to C++ and configuration')
  .option('-o, --output <dir>', 'Output directory', 'build')
  .option('--target <target>', 'Target platform', 'linux-aarch64')
  .action(async (path, options) => {
    const { build } = await import('./commands/build');
    await build(path, options);
  });

program
  .command('simulate [path]')
  .description('Run robot in simulation')
  .option('-b, --backend <backend>', 'Simulation backend', 'mujoco')
  .option('--headless', 'Run without GUI')
  .action(async (path, options) => {
    const { simulate } = await import('./commands/simulate');
    await simulate(path, options);
  });

program
  .command('deploy [path]')
  .description('Deploy robot to target hardware')
  .option('--target <host>', 'Target hostname or IP')
  .option('--user <user>', 'SSH username', 'hefaos')
  .action(async (path, options) => {
    const { deploy } = await import('./commands/deploy');
    await deploy(path, options);
  });

program
  .command('monitor [host]')
  .description('Connect to robot and display telemetry')
  .option('-p, --port <port>', 'Telemetry port', '9090')
  .action(async (host, options) => {
    const { monitor } = await import('./commands/monitor');
    await monitor(host, options);
  });

// ─────────────────────────────────────────────────────────────────────────────
// Run
// ─────────────────────────────────────────────────────────────────────────────

program.parse();
