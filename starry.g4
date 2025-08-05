grammar starry;

// 解析器规则
program
    : statement* EOF
    ;

statement
    : classDeclaration
    | functionDeclaration
    | variableDeclaration
    | expressionStatement
    | ifStatement
    | forStatement
    | whileStatement
    | returnStatement
    | block
    ;

classDeclaration
    : 'class' IDENTIFIER ('extends' IDENTIFIER)? '{' classBody '}'
    ;

classBody
    : classMember*
    ;

classMember
    : variableDeclaration
    | functionDeclaration
    | constructorDeclaration
    ;

constructorDeclaration
    : 'constructor' '(' parameterList? ')' block
    ;

functionDeclaration
    : 'fun' IDENTIFIER '(' parameterList? ')' (':' type)? block
    ;

parameterList
    : parameter (',' parameter)*
    ;

parameter
    : IDENTIFIER ':' type
    ;

variableDeclaration
    : ('var' | 'val') IDENTIFIER (':' type)? ('=' expression)? ';'
    ;

type
    : IDENTIFIER
    | IDENTIFIER '?'
    | IDENTIFIER '<' typeList '>'
    | '(' typeList ')'
    | '[' type ';' INTEGER_LITERAL ']'
    ;

typeList
    : type (',' type)*
    ;

block
    : '{' statement* '}'
    ;

ifStatement
    : 'if' '(' expression ')' statement ('else' statement)?
    ;

forStatement
    : 'for' '(' (variableDeclaration | expressionStatement | ';') expression? ';' expression? ')' statement
    ;

whileStatement
    : 'while' '(' expression ')' statement
    ;

returnStatement
    : 'return' expression? ';'
    ;

expressionStatement
    : expression ';'
    ;

expression
    : assignmentExpression
    ;

assignmentExpression
    : logicalOrExpression
    | unaryExpression assignmentOperator assignmentExpression
    ;

assignmentOperator
    : '=' | '+=' | '-=' | '*=' | '/=' | '%=' | '&=' | '|=' | '^=' | '<<=' | '>>='
    ;

logicalOrExpression
    : logicalAndExpression
    | logicalOrExpression '||' logicalAndExpression
    ;

logicalAndExpression
    : equalityExpression
    | logicalAndExpression '&&' equalityExpression
    ;

equalityExpression
    : relationalExpression
    | equalityExpression ('==' | '!=' | '===' | '!==') relationalExpression
    ;

relationalExpression
    : additiveExpression
    | relationalExpression ('<' | '>' | '<=' | '>=') additiveExpression
    | relationalExpression 'is' type
    | relationalExpression 'as' type
    | relationalExpression 'as?' type
    ;

additiveExpression
    : multiplicativeExpression
    | additiveExpression ('+' | '-') multiplicativeExpression
    ;

multiplicativeExpression
    : unaryExpression
    | multiplicativeExpression ('*' | '/' | '%') unaryExpression
    ;

unaryExpression
    : postfixExpression
    | ('!' | '-' | '~' | '++' | '--') unaryExpression
    ;

postfixExpression
    : primaryExpression
    | postfixExpression '.' IDENTIFIER
    | postfixExpression '?.' IDENTIFIER
    | postfixExpression '[' expression ']'
    | postfixExpression '(' argumentList? ')'
    | postfixExpression ('++' | '--')
    ;

primaryExpression
    : IDENTIFIER
    | literal
    | '(' expression ')'
    | 'this'
    | 'super'
    | 'null'
    ;

argumentList
    : expression (',' expression)*
    ;

literal
    : INTEGER_LITERAL
    | FLOAT_LITERAL
    | STRING_LITERAL
    | CHAR_LITERAL
    | 'true'
    | 'false'
    ;

// 词法规则
CLASS: 'class';
STRUCT: 'struct';
ENUM: 'enum';
UNION: 'union';
TYPEDEF: 'typedef';
USING: 'using';
IF: 'if';
ELSE: 'else';
SWITCH: 'switch';
CASE: 'case';
DEFAULT: 'default';
FOR: 'for';
WHILE: 'while';
DO: 'do';
BREAK: 'break';
CONTINUE: 'continue';
RETURN: 'return';
GOTO: 'goto';
TRY: 'try';
CATCH: 'catch';
THROW: 'throw';
CONST: 'const';
VOLATILE: 'volatile';
STATIC: 'static';
EXTERN: 'extern';
INLINE: 'inline';
VIRTUAL: 'virtual';
EXPLICIT: 'explicit';
FRIEND: 'friend';
MUTABLE: 'mutable';
PUBLIC: 'public';
PRIVATE: 'private';
PROTECTED: 'protected';
NEW: 'new';
DELETE: 'delete';
SIZEOF: 'sizeof';
TEMPLATE: 'template';
TYPENAME: 'typename';
NAMESPACE: 'namespace';
TRUE: 'true';
FALSE: 'false';
NULL_LITERAL: 'null';
VAR: 'var';
VAL: 'val';
IS: 'is';
AS: 'as';
EXTENSION: 'extension';

IDENTIFIER
    : [a-zA-Z_][a-zA-Z0-9_]*
    ;

INTEGER_LITERAL
    : [0-9]+
    | '0x' [0-9a-fA-F]+
    | '0b' [01]+
    ;

FLOAT_LITERAL
    : [0-9]+ '.' [0-9]* ([eE] [+-]? [0-9]+)?
    | '.' [0-9]+ ([eE] [+-]? [0-9]+)?
    | [0-9]+ [eE] [+-]? [0-9]+
    ;

STRING_LITERAL
    : '"' (~["\\\r\n] | '\\' .)* '"'
    ;

CHAR_LITERAL
    : '\'' (~['\\\r\n] | '\\' .) '\''
    ;

COMMENT
    : '//' ~[\r\n]* -> skip
    | '/*' .*? '*/' -> skip
    ;

WS
    : [ \t\r\n]+ -> skip
    ;