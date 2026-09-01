/**
 * @hefaos/compiler - TSX to C++ code generator
 *
 * @packageDocumentation
 */

import type { RobotDefinition, CompilationResult } from '@hefaos/types';

// ─────────────────────────────────────────────────────────────────────────────
// Public API
// ─────────────────────────────────────────────────────────────────────────────

export interface CompileOptions {
  format?: 'cpp' | 'yaml' | 'flatbuffer' | 'all';
  outputDir?: string;
  namespace?: string;
}

/**
 * Compile a robot definition to target format(s)
 */
export async function compile(
  input: string,
  options: CompileOptions = {}
): Promise<CompilationResult> {
  // TODO: Implement compilation
  const _format = options.format ?? 'all';

  return {
    cpp: '// Generated C++ code placeholder',
    yaml: '# Generated YAML config placeholder',
    flatbuffer: '// Generated FlatBuffer schema placeholder',
    errors: [],
    warnings: [],
  };
}

/**
 * Parse a robot definition from TSX source
 */
export function parseRobotDefinition(_source: string): RobotDefinition {
  // TODO: Implement parsing
  return {
    name: 'Placeholder',
    components: [],
    tasks: [],
  };
}

// ─────────────────────────────────────────────────────────────────────────────
// Generators
// ─────────────────────────────────────────────────────────────────────────────

export { CppGenerator } from './generators/cpp';
export { YamlGenerator } from './generators/yaml';
export { FlatBufferGenerator } from './generators/flatbuffer';

// ─────────────────────────────────────────────────────────────────────────────
// Parser
// ─────────────────────────────────────────────────────────────────────────────

export { Parser } from './parser';
export { Transformer } from './transformer';
