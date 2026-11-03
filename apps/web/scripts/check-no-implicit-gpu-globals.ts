import ts from 'typescript'

const browserConstants = new Set([
  'GPUBufferUsage', 'GPUMapMode', 'GPUShaderStage', 'GPUTextureUsage', 'GPUColorWrite',
])

// Resolve lexical bindings, not the DOM declarations of property names: webgpu.globals
// deliberately has the same property types as globalThis without being globalThis.
export function implicitGpuGlobals(program: ts.Program, source: ts.SourceFile): string[] {
  const checker = program.getTypeChecker()
  const violations: string[] = []
  const hasLocalValue = (node: ts.Identifier): boolean => {
    const symbol = ts.isShorthandPropertyAssignment(node.parent)
      ? checker.getShorthandAssignmentValueSymbol(node.parent)
      : checker.getSymbolAtLocation(node)
    return !!symbol?.declarations?.some(declaration => {
      if (declaration.getSourceFile().isDeclarationFile
        || ts.isInterfaceDeclaration(declaration) || ts.isTypeAliasDeclaration(declaration)
        || ts.findAncestor(declaration, node =>
          (ts.isModuleDeclaration(node) && !!(node.flags & ts.NodeFlags.GlobalAugmentation))
          || (ts.canHaveModifiers(node) && !!ts.getModifiers(node)?.some(modifier => modifier.kind === ts.SyntaxKind.DeclareKeyword)))) return false
      if (ts.isImportSpecifier(declaration)) {
        return !declaration.isTypeOnly && !declaration.parent.parent.isTypeOnly
      }
      if (ts.isImportEqualsDeclaration(declaration)) return !declaration.isTypeOnly
      if (ts.isImportClause(declaration)) return !declaration.isTypeOnly
      if (ts.isNamespaceImport(declaration)) return !declaration.parent.isTypeOnly
      return !!(symbol.flags & ts.SymbolFlags.Value)
    })
  }
  const report = (node: ts.Node, name: string) => {
    const { line, character } = source.getLineAndCharacterOfPosition(node.getStart(source))
    violations.push(`${source.fileName}:${line + 1}:${character + 1}: implicit browser WebGPU global ${name}; import or bind it from webgpu.globals`)
  }
  const checkKey = (node: ts.Node, key: ts.Node | undefined) => {
    if (key && ts.isComputedPropertyName(key)) key = key.expression
    if (key && (ts.isIdentifier(key) || ts.isStringLiteralLike(key)) && browserConstants.has(key.text)) report(node, key.text)
  }
  const isGlobalThis = (node: ts.Expression): boolean => {
    while (ts.isParenthesizedExpression(node) || ts.isAsExpression(node) || ts.isTypeAssertionExpression(node) || ts.isNonNullExpression(node) || ts.isSatisfiesExpression(node)) node = node.expression
    return ts.isIdentifier(node) && node.text === 'globalThis' && !hasLocalValue(node)
  }
  const visit = (node: ts.Node) => {
    if ((ts.isTypeNode(node) && !ts.isExpressionWithTypeArguments(node))
      || (ts.isHeritageClause(node) && (ts.isInterfaceDeclaration(node.parent) || node.token === ts.SyntaxKind.ImplementsKeyword))
      || ts.isImportDeclaration(node) || ts.isExportDeclaration(node)) return
    if (ts.isBindingElement(node) && ts.isObjectBindingPattern(node.parent)) {
      const declaration = node.parent.parent
      if ((ts.isVariableDeclaration(declaration) || ts.isParameter(declaration))
        && declaration.initializer && isGlobalThis(declaration.initializer)) {
        checkKey(node, node.propertyName ?? node.name)
      }
    }
    if (ts.isBinaryExpression(node) && node.operatorToken.kind === ts.SyntaxKind.EqualsToken
      && ts.isObjectLiteralExpression(node.left) && isGlobalThis(node.right)) {
      for (const property of node.left.properties) checkKey(property, property.name)
    }
    if (ts.isPropertyAccessExpression(node) && isGlobalThis(node.expression) && browserConstants.has(node.name.text)) {
      report(node, node.name.text)
    }
    if (ts.isElementAccessExpression(node) && isGlobalThis(node.expression)
      && ts.isStringLiteralLike(node.argumentExpression) && browserConstants.has(node.argumentExpression.text)) {
      report(node, node.argumentExpression.text)
    }
    if (ts.isIdentifier(node) && browserConstants.has(node.text)) {
      const parent = node.parent
      const isName = ('name' in parent && parent.name === node && !ts.isShorthandPropertyAssignment(parent))
        || (ts.isBindingElement(parent) && parent.propertyName === node)
        || ts.isLabeledStatement(parent) || ts.isBreakStatement(parent) || ts.isContinueStatement(parent)
      if (!isName && !hasLocalValue(node)) report(node, node.text)
    }
    ts.forEachChild(node, visit)
  }
  visit(source)
  return violations
}
