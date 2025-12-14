#include <iostream>
#include <string>
#include <vector>
#include <map>
#include <memory>
#include <functional>
#include <sstream>
#include <fstream>
#include <chrono>
#include <stdexcept>
#include <cctype>
#include <cstdlib>
#include <cmath>
#include "gc_value.h"
#include "bytecode.h"
#include "slime_gc.h"

// 词法单元类型枚举
enum TokenType {
    TOKEN_EOF,
    TOKEN_KEYWORD,
    TOKEN_IDENTIFIER,
    TOKEN_NUMBER,
    TOKEN_STRING,
    TOKEN_PUNCTUATION,
    TOKEN_DIRECTIVE
};

struct Token {
    TokenType type;
    std::string value;
    int line;
    int column;
};

// 词法分析器类
class Lexer {
public:
    Lexer(const std::string& input) : input(input), position(0), line(1), column(1) {}
    
    Token getNextToken();
    
private:
    std::string input;
    size_t position;
    int line;
    int column;
    
    char advance();
    char peek();
    bool isAtEnd();
    void skipWhitespace();
    void skipComment();
    Token identifier();
    Token string();
    Token number();
    Token directive();
};

// 语法分析器 - AST节点定义
enum NodeType {
    NODE_PROGRAM,
    NODE_STATEMENT,
    NODE_CALL,
    NODE_STRING_LITERAL,
    NODE_NUMBER_LITERAL,
    NODE_IDENTIFIER,
    NODE_DIRECTIVE,
    NODE_EXPRESSION,
    NODE_OPERATOR,
    NODE_IF_STATEMENT,
    NODE_WHILE_STATEMENT,
    NODE_FOR_STATEMENT,
    NODE_BREAK_STATEMENT,
    NODE_CONTINUE_STATEMENT,
    NODE_COMPARISON,
    NODE_LOGICAL_OPERATOR,
    NODE_ASSIGN,
    NODE_BLOCK
};

struct ASTNode {
    NodeType type;
    std::string value;
    std::vector<std::unique_ptr<ASTNode>> children;
    
    ASTNode(NodeType type, const std::string& value = "") : type(type), value(value) {}
    
    // 添加子节点
    void addChild(std::unique_ptr<ASTNode> child) {
        children.push_back(std::move(child));
    }
    
    // 获取子节点（用于兼容现有代码）
    ASTNode* getChild(size_t index) const {
        if (index < children.size()) {
            return children[index].get();
        }
        return nullptr;
    }
    
    // 获取子节点数量
    size_t getChildCount() const {
        return children.size();
    }
    
    // 获取子节点的范围（用于范围for循环）
    std::vector<ASTNode*> getChildren() const {
        std::vector<ASTNode*> result;
        result.reserve(children.size());
        for (const auto& child : children) {
            result.push_back(child.get());
        }
        return result;
    }
};

// 语法分析器类
class Parser {
public:
    Parser(Lexer& lexer) : lexer(lexer), currentToken(lexer.getNextToken()) {}
    
    ASTNode* parse();
    
private:
    Lexer& lexer;
    Token currentToken;
    
    void eat(TokenType type);
    ASTNode* program();
    ASTNode* statement();
    ASTNode* call();
    ASTNode* expression();
    ASTNode* expr();
    ASTNode* term();
    ASTNode* factor();
    ASTNode* directive();
    bool isAtEndOfLine();
    // 新增控制流语句解析函数
    ASTNode* parseIfStatement();
    ASTNode* parseWhileStatement();
    ASTNode* parseForStatement();
    ASTNode* parseBreakStatement();
    ASTNode* parseContinueStatement();
};

// 解释器类
class Interpreter {
public:
    Interpreter() {
        initBaselib();
    }
    
    void execute(const std::string& code);
    void executeContent(const std::string& content);
    
private:
    std::map<std::string, std::function<void(const std::vector<std::string>&)>> functions;
    std::map<std::string, GCValue> variables;
    
    void initBaselib();
    void interpret(ASTNode* node);
    GCValue evaluateExpression(ASTNode* node);
};

// 字节码生成器类
class BytecodeGenerator {
public:
    BytecodeGenerator() : writer(bytecode) {}
    Bytecode generate(ASTNode* ast);
    
private:
    Bytecode bytecode;
    BytecodeWriter writer;
    std::map<std::string, uint16_t> stringConstants;
    std::map<double, uint16_t> numberConstants;
    std::map<std::string, uint16_t> generalConstants;
    std::map<std::string, uint16_t> functionNames;
    std::vector<std::pair<size_t, size_t>> loopStack; // 存储循环开始位置和结束跳转位置
    
    // 生成节点的字节码
    void generateNode(ASTNode* node);
    
    // 生成语句的字节码
    void generateStatement(ASTNode* node);
    
    // 生成控制流语句的字节码
    void generateIfStatement(ASTNode* node);
    void generateWhileStatement(ASTNode* node);
    void generateForStatement(ASTNode* node);
    void generateBreakStatement(ASTNode* node);
    void generateContinueStatement(ASTNode* node);
    
    // 生成赋值语句的字节码
    void generateAssignment(ASTNode* node);
    
    // 生成函数调用的字节码
    void generateCall(ASTNode* node);
    
    // 生成表达式的字节码
    void generateExpression(ASTNode* node);
    
    // 添加常量到相应的池中并返回索引
    uint16_t addStringConstant(const std::string& str);
    uint16_t addNumberConstant(double num);
    uint16_t addGeneralConstant(const std::string& constant);
    uint16_t addFunctionName(const std::string& functionName);
};

// 虚拟机类
class BytecodeVM {
public:
    void execute(const Bytecode& bytecode);
    
private:
    std::vector<GCValue> stack;
    std::map<std::string, std::function<void(const std::vector<std::string>&)>> functions;
    std::map<std::string, GCValue> variables;
    size_t programCounter;
    
    void initBaselib();
    GCValue pop();
    void push(const GCValue& value);
    void executeInstruction(const Bytecode& bytecode);
};

// 编译字节码为可执行文件
void compileBytecodeToExe(const Bytecode& bytecode, const std::string& outputFilename);

// 词法分析器实现
char Lexer::advance() {
    column++;
    return input[position++];
}

char Lexer::peek() {
    return isAtEnd() ? '\0' : input[position];
}

bool Lexer::isAtEnd() {
    return position >= input.size();
}

void Lexer::skipWhitespace() {
    while (!isAtEnd() && isspace(input[position])) {
        if (input[position] == '\n') {
            line++;
            column = 1;
        }
        advance();
    }
}

void Lexer::skipComment() {
    while (!isAtEnd() && input[position] != '\n') {
        advance();
    }
    if (!isAtEnd()) {
        advance();
    }
}

Token Lexer::identifier() {
    size_t start = position;
    while (!isAtEnd() && (isalnum(input[position]) || input[position] == '_' || input[position] == '.')) {
        advance();
    }
    
    std::string value = input.substr(start, position - start);
    std::vector<std::string> keywords = {"cra", "cre", "use", "del", "if", "else", "while", "for", "break", "continue"};
    for (const auto& keyword : keywords) {
        if (value == keyword) {
            return {TOKEN_KEYWORD, value, line, column};
        }
    }
    
    return {TOKEN_IDENTIFIER, value, line, column};
}

Token Lexer::string() {
    advance();
    size_t start = position;
    
    while (!isAtEnd() && input[position] != '"') {
        if (input[position] == '\\') {
            advance();
        }
        advance();
    }
    
    if (isAtEnd()) {
        throw std::runtime_error("Unterminated string");
    }
    
    std::string value = input.substr(start, position - start);
    advance();
    return {TOKEN_STRING, value, line, column};
}

Token Lexer::number() {
    size_t start = position;
    while (!isAtEnd() && (isdigit(input[position]) || input[position] == '.')) {
        advance();
    }
    
    std::string value = input.substr(start, position - start);
    return {TOKEN_NUMBER, value, line, column};
}

Token Lexer::directive() {
    size_t start = position;
    advance(); // 跳过#
    
    while (!isAtEnd() && !isspace(input[position]) && input[position] != ';') {
        advance();
    }
    
    std::string value = input.substr(start, position - start);
    return {TOKEN_DIRECTIVE, value, line, column};
}

Token Lexer::getNextToken() {
    while (!isAtEnd()) {
        char c = input[position];
        
        if (isspace(c)) {
            skipWhitespace();
            continue;
        }
        
        if (c == '#') {
            return directive();
        }
        
        if (c == '"') {
            return string();
        }
        
        if (isdigit(c)) {
            return number();
        }
        
        if (isalpha(c) || c == '_') {
            return identifier();
        }
        
        // 处理数学运算符
        if (c == '+' || c == '-' || c == '*' || c == '/' || c == '%') {
            char op = c;
            advance();
            
            // 处理除法注释
            if (op == '/' && peek() == '/') {
                skipComment();
                return getNextToken();
            }
            
            return {TOKEN_PUNCTUATION, std::string(1, op), line, column};
        }
        
        // 处理比较运算符
        if (c == '=' || c == '!' || c == '<' || c == '>') {
            char op1 = c;
            advance();
            
            if ((op1 == '=' || op1 == '!' || op1 == '<' || op1 == '>') && peek() == '=') {
                char op2 = advance();
                return {TOKEN_PUNCTUATION, std::string(1, op1) + op2, line, column};
            }
            
            return {TOKEN_PUNCTUATION, std::string(1, op1), line, column};
        }
        
        // 处理逻辑运算符
        if (c == '&' || c == '|') {
            char op1 = c;
            advance();
            
            if (peek() == op1) {
                advance();
                return {TOKEN_PUNCTUATION, std::string(2, op1), line, column};
            }
            
            throw std::runtime_error("Invalid operator at line " + std::to_string(line));
        }
        
        advance();
        return {TOKEN_PUNCTUATION, std::string(1, c), line, column};
    }
    
    return {TOKEN_EOF, "", line, column};
}

// 语法分析器实现
ASTNode* Parser::parse() {
    return program();
}

void Parser::eat(TokenType type) {
    if (currentToken.type == type) {
        currentToken = lexer.getNextToken();
    } else {
        throw std::runtime_error("Syntax Error: Expected token type " + std::to_string(type) + 
                               " but got " + std::to_string(currentToken.type) + 
                               " at line " + std::to_string(currentToken.line));
    }
}

ASTNode* Parser::program() {
    auto program = new ASTNode(NODE_PROGRAM);
    
    while (currentToken.type != TOKEN_EOF) {
        if (currentToken.type == TOKEN_DIRECTIVE) {
            program->addChild(std::unique_ptr<ASTNode>(directive()));
        } else {
            program->addChild(std::unique_ptr<ASTNode>(statement()));
        }
    }
    
    return program;
}

ASTNode* Parser::statement() {
    if (currentToken.type == TOKEN_KEYWORD) {
        if (currentToken.value == "if") {
            return parseIfStatement();
        } else if (currentToken.value == "while") {
            return parseWhileStatement();
        } else if (currentToken.value == "for") {
            return parseForStatement();
        } else if (currentToken.value == "break") {
            return parseBreakStatement();
        } else if (currentToken.value == "continue") {
            return parseContinueStatement();
        }
    }
    
    auto stmt = new ASTNode(NODE_STATEMENT);
    
    if (currentToken.type == TOKEN_KEYWORD) {
        stmt->value = currentToken.value;
        eat(TOKEN_KEYWORD);
        
        if (stmt->value == "use") {
            stmt->addChild(std::unique_ptr<ASTNode>(call()));
        } else if (stmt->value == "cra" || stmt->value == "del") {
            if (currentToken.type == TOKEN_IDENTIFIER) {
                stmt->addChild(std::unique_ptr<ASTNode>(new ASTNode(NODE_IDENTIFIER, currentToken.value)));
                eat(TOKEN_IDENTIFIER);
            }
            
            // 处理函数体的开始括号 {}
            if (currentToken.type == TOKEN_PUNCTUATION && currentToken.value == "{") {
                eat(TOKEN_PUNCTUATION); // 吃掉左括号
                
                // 解析函数体内部的语句
                while (currentToken.type != TOKEN_EOF && 
                       !(currentToken.type == TOKEN_PUNCTUATION && currentToken.value == "}")) {
                    stmt->addChild(std::unique_ptr<ASTNode>(statement()));
                }
                
                if (currentToken.type == TOKEN_PUNCTUATION && currentToken.value == "}") {
                    eat(TOKEN_PUNCTUATION); // 吃掉右括号
                }
            }
        } else if (stmt->value == "cre") {
            // 处理接口创建语法: cre sta.interface "Example.Print" int Out "System.Output.Print"
            while (currentToken.type != TOKEN_EOF && 
                   !(currentToken.type == TOKEN_KEYWORD) && 
                   !(currentToken.type == TOKEN_PUNCTUATION && currentToken.value == "}")) {
                if (currentToken.type == TOKEN_STRING || currentToken.type == TOKEN_IDENTIFIER) {
                    stmt->addChild(std::unique_ptr<ASTNode>(expression()));
                } else {
                    // 跳过其他标记（如标点符号和关键字）
                    eat(currentToken.type);
                }
            }
        }
    }
    
    return stmt;
}

// 解析if语句
ASTNode* Parser::parseIfStatement() {
    auto ifStmt = new ASTNode(NODE_IF_STATEMENT);
    eat(TOKEN_KEYWORD); // 吃掉if
    
    // 解析条件表达式
    if (currentToken.type == TOKEN_PUNCTUATION && currentToken.value == "(") {
        eat(TOKEN_PUNCTUATION); // 吃掉(
        ifStmt->addChild(std::unique_ptr<ASTNode>(expression()));
        if (currentToken.type == TOKEN_PUNCTUATION && currentToken.value == ")") {
            eat(TOKEN_PUNCTUATION); // 吃掉)
        }
    }
    
    // 解析if体，创建块节点
    auto ifBlock = new ASTNode(NODE_BLOCK);
    if (currentToken.type == TOKEN_PUNCTUATION && currentToken.value == "{") {
        eat(TOKEN_PUNCTUATION); // 吃掉{
        while (currentToken.type != TOKEN_EOF && 
               !(currentToken.type == TOKEN_PUNCTUATION && currentToken.value == "}")) {
            ifBlock->addChild(std::unique_ptr<ASTNode>(statement()));
        }
        if (currentToken.type == TOKEN_PUNCTUATION && currentToken.value == "}") {
            eat(TOKEN_PUNCTUATION); // 吃掉}
        }
    }
    ifStmt->addChild(std::unique_ptr<ASTNode>(ifBlock));
    
    // 解析else
    if (currentToken.type == TOKEN_KEYWORD && currentToken.value == "else") {
        eat(TOKEN_KEYWORD); // 吃掉else
        if (currentToken.type == TOKEN_PUNCTUATION && currentToken.value == "{") {
            eat(TOKEN_PUNCTUATION); // 吃掉{
            auto elseBlock = new ASTNode(NODE_BLOCK);
            while (currentToken.type != TOKEN_EOF && 
                   !(currentToken.type == TOKEN_PUNCTUATION && currentToken.value == "}")) {
                elseBlock->addChild(std::unique_ptr<ASTNode>(statement()));
            }
            if (currentToken.type == TOKEN_PUNCTUATION && currentToken.value == "}") {
                eat(TOKEN_PUNCTUATION); // 吃掉}
            }
            ifStmt->addChild(std::unique_ptr<ASTNode>(elseBlock));
        } else if (currentToken.type == TOKEN_KEYWORD && currentToken.value == "if") {
            // 解析else if - 创建块节点包含else if语句
            auto elseIfBlock = new ASTNode(NODE_BLOCK);
            elseIfBlock->addChild(std::unique_ptr<ASTNode>(parseIfStatement()));
            ifStmt->addChild(std::unique_ptr<ASTNode>(elseIfBlock));
        }
    }
    
    return ifStmt;
}

// 解析while语句
ASTNode* Parser::parseWhileStatement() {
    auto whileStmt = new ASTNode(NODE_WHILE_STATEMENT);
    eat(TOKEN_KEYWORD); // 吃掉while
    
    // 解析条件表达式
    if (currentToken.type == TOKEN_PUNCTUATION && currentToken.value == "(") {
        eat(TOKEN_PUNCTUATION); // 吃掉(
        whileStmt->addChild(std::unique_ptr<ASTNode>(expression()));
        if (currentToken.type == TOKEN_PUNCTUATION && currentToken.value == ")") {
            eat(TOKEN_PUNCTUATION); // 吃掉)
        }
    }
    
    // 解析循环体，创建块节点
    auto loopBlock = new ASTNode(NODE_BLOCK);
    if (currentToken.type == TOKEN_PUNCTUATION && currentToken.value == "{") {
        eat(TOKEN_PUNCTUATION); // 吃掉{
        while (currentToken.type != TOKEN_EOF && 
               !(currentToken.type == TOKEN_PUNCTUATION && currentToken.value == "}")) {
            loopBlock->addChild(std::unique_ptr<ASTNode>(statement()));
        }
        if (currentToken.type == TOKEN_PUNCTUATION && currentToken.value == "}") {
            eat(TOKEN_PUNCTUATION); // 吃掉}
        }
    }
    whileStmt->addChild(std::unique_ptr<ASTNode>(loopBlock));
    
    return whileStmt;
}

// 解析for语句
ASTNode* Parser::parseForStatement() {
    auto forStmt = new ASTNode(NODE_FOR_STATEMENT);
    eat(TOKEN_KEYWORD); // 吃掉for
    
    // 解析for循环的括号和内容
    if (currentToken.type == TOKEN_PUNCTUATION && currentToken.value == "(") {
        eat(TOKEN_PUNCTUATION); // 吃掉(
        
        // 解析初始化语句
        if (currentToken.type != TOKEN_PUNCTUATION || currentToken.value != ";") {
            forStmt->addChild(std::unique_ptr<ASTNode>(statement()));
        }
        
        // 跳过;
        if (currentToken.type == TOKEN_PUNCTUATION && currentToken.value == ";") {
            eat(TOKEN_PUNCTUATION);
        }
        
        // 解析条件表达式
        if (currentToken.type != TOKEN_PUNCTUATION || currentToken.value != ";") {
            forStmt->addChild(std::unique_ptr<ASTNode>(expression()));
        }
        
        // 跳过;
        if (currentToken.type == TOKEN_PUNCTUATION && currentToken.value == ";") {
            eat(TOKEN_PUNCTUATION);
        }
        
        // 解析迭代语句
        if (currentToken.type != TOKEN_PUNCTUATION || currentToken.value != ")") {
            forStmt->addChild(std::unique_ptr<ASTNode>(statement()));
        }
        
        // 吃掉)
        if (currentToken.type == TOKEN_PUNCTUATION && currentToken.value == ")") {
            eat(TOKEN_PUNCTUATION);
        }
    }
    
    // 解析循环体，创建块节点
    auto loopBlock = new ASTNode(NODE_BLOCK);
    if (currentToken.type == TOKEN_PUNCTUATION && currentToken.value == "{") {
        eat(TOKEN_PUNCTUATION); // 吃掉{
        while (currentToken.type != TOKEN_EOF && 
               !(currentToken.type == TOKEN_PUNCTUATION && currentToken.value == "}")) {
            loopBlock->addChild(std::unique_ptr<ASTNode>(statement()));
        }
        if (currentToken.type == TOKEN_PUNCTUATION && currentToken.value == "}") {
            eat(TOKEN_PUNCTUATION); // 吃掉}
        }
    }
    forStmt->addChild(std::unique_ptr<ASTNode>(loopBlock));
    
    return forStmt;
}

// 解析break语句
ASTNode* Parser::parseBreakStatement() {
    eat(TOKEN_KEYWORD); // 吃掉break
    return new ASTNode(NODE_BREAK_STATEMENT);
}

// 解析continue语句
ASTNode* Parser::parseContinueStatement() {
    eat(TOKEN_KEYWORD); // 吃掉continue
    return new ASTNode(NODE_CONTINUE_STATEMENT);
}

ASTNode* Parser::call() {
    auto call = new ASTNode(NODE_CALL);
    
    if (currentToken.type == TOKEN_IDENTIFIER) {
        call->value = currentToken.value;
        eat(TOKEN_IDENTIFIER);
    }
    
    // 只解析一个参数，这个参数可以是完整的表达式
    if (currentToken.type != TOKEN_EOF && 
        !(currentToken.type == TOKEN_KEYWORD) && 
        !(currentToken.type == TOKEN_PUNCTUATION && currentToken.value == "}")) {
        call->addChild(std::unique_ptr<ASTNode>(expression()));
    }
    
    return call;
}

ASTNode* Parser::expression() {
    if (currentToken.type == TOKEN_STRING) {
        auto node = new ASTNode(NODE_STRING_LITERAL, currentToken.value);
        eat(TOKEN_STRING);
        return node;
    } else if (currentToken.type == TOKEN_NUMBER || 
               currentToken.type == TOKEN_PUNCTUATION && currentToken.value == "(") {
        // 直接解析完整的数学表达式，包括加减乘除和括号
        ASTNode* exprNode = new ASTNode(NODE_EXPRESSION);
        exprNode->addChild(std::unique_ptr<ASTNode>(expr()));
        return exprNode;
    } else if (currentToken.type == TOKEN_IDENTIFIER) {
        auto identifier = new ASTNode(NODE_IDENTIFIER, currentToken.value);
        eat(TOKEN_IDENTIFIER);
        
        // 检查是否是赋值语句
        if (currentToken.type == TOKEN_PUNCTUATION && currentToken.value == "=") {
            eat(TOKEN_PUNCTUATION); // 吃掉等号
            
            // 创建赋值节点
            auto assignNode = new ASTNode(NODE_ASSIGN);
            assignNode->addChild(std::unique_ptr<ASTNode>(identifier)); // 左值是标识符
            
            // 解析右值表达式
            ASTNode* rightExpr;
            if (currentToken.type == TOKEN_STRING) {
                rightExpr = new ASTNode(NODE_STRING_LITERAL, currentToken.value);
                eat(TOKEN_STRING);
            } else {
                // 解析数学表达式作为右值
                ASTNode* exprNode = new ASTNode(NODE_EXPRESSION);
                exprNode->addChild(std::unique_ptr<ASTNode>(expr()));
                rightExpr = exprNode;
            }
            
            assignNode->addChild(std::unique_ptr<ASTNode>(rightExpr)); // 右值是表达式
            return assignNode;
        }
        
        return identifier;
    }
    
    throw std::runtime_error("Syntax Error: Unexpected token at line " + std::to_string(currentToken.line));
}

// 表达式解析：处理加减运算
ASTNode* Parser::expr() {
    ASTNode* left = term();
    
    while (currentToken.type == TOKEN_PUNCTUATION && (currentToken.value == "+" || currentToken.value == "-")) {
        auto op = new ASTNode(NODE_OPERATOR, currentToken.value);
        eat(TOKEN_PUNCTUATION);
        op->addChild(std::unique_ptr<ASTNode>(left));
        op->addChild(std::unique_ptr<ASTNode>(term()));
        left = op;
    }
    
    return left;
}

// 项解析：处理乘除取余运算
ASTNode* Parser::term() {
    ASTNode* left = factor();
    
    while (currentToken.type == TOKEN_PUNCTUATION && (currentToken.value == "*" || currentToken.value == "/" || currentToken.value == "%")) {
        std::string opValue = currentToken.value;
        eat(TOKEN_PUNCTUATION); // 吃掉运算符
        ASTNode* right = factor();
        
        auto op = new ASTNode(NODE_OPERATOR, opValue);
        op->addChild(std::unique_ptr<ASTNode>(left));
        op->addChild(std::unique_ptr<ASTNode>(right));
        left = op;
    }
    
    return left;
}

// 因子解析：处理数字、标识符和括号
ASTNode* Parser::factor() {
    if (currentToken.type == TOKEN_NUMBER) {
        auto node = new ASTNode(NODE_NUMBER_LITERAL, currentToken.value);
        eat(TOKEN_NUMBER);
        return node;
    } else if (currentToken.type == TOKEN_IDENTIFIER) {
        auto node = new ASTNode(NODE_IDENTIFIER, currentToken.value);
        eat(TOKEN_IDENTIFIER);
        return node;
    } else if (currentToken.type == TOKEN_PUNCTUATION && currentToken.value == "(") {
        eat(TOKEN_PUNCTUATION); // 吃掉左括号
        auto node = expr();
        if (currentToken.type == TOKEN_PUNCTUATION && currentToken.value == ")") {
            eat(TOKEN_PUNCTUATION); // 吃掉右括号
        } else {
            throw std::runtime_error("Syntax Error: Missing closing parenthesis at line " + std::to_string(currentToken.line));
        }
        return node;
    }
    
    throw std::runtime_error("Syntax Error: Unexpected token at line " + std::to_string(currentToken.line));
}

ASTNode* Parser::directive() {
    auto directive = new ASTNode(NODE_DIRECTIVE, currentToken.value);
    eat(TOKEN_DIRECTIVE);
    
    while (!isAtEndOfLine() && currentToken.type != TOKEN_EOF) {
        if (currentToken.type == TOKEN_STRING || currentToken.type == TOKEN_IDENTIFIER || 
            currentToken.type == TOKEN_NUMBER) {
            directive->addChild(std::unique_ptr<ASTNode>(expression()));
        } else {
            eat(currentToken.type);
        }
    }
    
    return directive;
}

bool Parser::isAtEndOfLine() {
    return currentToken.type == TOKEN_PUNCTUATION && currentToken.value == ";" || 
           currentToken.type == TOKEN_KEYWORD ||
           currentToken.type == TOKEN_DIRECTIVE ||
           currentToken.type == TOKEN_EOF;
}

// 解释器实现
void Interpreter::initBaselib() {
    // 注册基础库函数
    functions["System.Output.Print"] = [](const std::vector<std::string>& args) {
        for (const auto& arg : args) {
            std::cout << arg;
        }
        std::cout << std::endl;
    };
    
    functions["System.Output.Println"] = [](const std::vector<std::string>& args) {
        for (const auto& arg : args) {
            std::cout << arg;
        }
        std::cout << std::endl;
    };
    
    functions["System.Input.Read"] = [](const std::vector<std::string>& args) {
        std::string input;
        std::cin >> input;
        std::cout << input;
    };
    
    functions["System.Input.ReadLine"] = [](const std::vector<std::string>& args) {
        std::string input;
        std::getline(std::cin, input);
        std::cout << input;
    };
    
    // 添加时间相关函数
    functions["System.Time.Now"] = [](const std::vector<std::string>& args) {
        auto now = std::chrono::high_resolution_clock::now();
        auto duration = now.time_since_epoch();
        auto milliseconds = std::chrono::duration_cast<std::chrono::milliseconds>(duration).count();
        std::cout << milliseconds;
    };
    
    // 数学函数
    functions["System.Math.Add"] = [](const std::vector<std::string>& args) {
        if (args.size() >= 2) {
            double a = std::stod(args[0]);
            double b = std::stod(args[1]);
            std::cout << (a + b);
        }
    };
    
    functions["System.Math.Subtract"] = [](const std::vector<std::string>& args) {
        if (args.size() >= 2) {
            double a = std::stod(args[0]);
            double b = std::stod(args[1]);
            std::cout << (a - b);
        }
    };
    
    functions["System.Math.Multiply"] = [](const std::vector<std::string>& args) {
        if (args.size() >= 2) {
            double a = std::stod(args[0]);
            double b = std::stod(args[1]);
            std::cout << (a * b);
        }
    };
    
    functions["System.Math.Divide"] = [](const std::vector<std::string>& args) {
        if (args.size() >= 2) {
            double a = std::stod(args[0]);
            double b = std::stod(args[1]);
            if (b != 0) {
                std::cout << (a / b);
            } else {
                std::cout << "Error: Division by zero";
            }
        }
    };
    
    functions["System.Math.Modulo"] = [](const std::vector<std::string>& args) {
        if (args.size() >= 2) {
            double a = std::stod(args[0]);
            double b = std::stod(args[1]);
            if (b != 0) {
                std::cout << fmod(a, b);
            } else {
                std::cout << "Error: Modulo by zero";
            }
        }
    };
}

void Interpreter::execute(const std::string& code) {
    Lexer lexer(code);
    Parser parser(lexer);
    ASTNode* ast = parser.parse();
    
    // 注册变量到GCValue
    GCValue::registerVariables(variables);
    
    // 执行解释
    interpret(ast);
    
    // 执行垃圾回收
    GCValue::collectGarbage();
    
    // 注销VM
    GCValue::unregisterVM();
    
    delete ast;
}

void Interpreter::executeContent(const std::string& content) {
    // 使用与execute相同的AST解析系统来处理引入的文件内容
    // 这样可以确保解析的一致性并避免手动解析的错误
    Lexer lexer(content);
    Parser parser(lexer);
    ASTNode* ast = parser.parse();
    
    // 注册VM变量以便GC可以跟踪根对象
    GCValue::registerVariables(variables);
    
    interpret(ast);
    delete ast;
    
    // 执行垃圾回收
    GCValue::collectGarbage();
}

void Interpreter::interpret(ASTNode* node) {
    if (!node) return;
    
    switch (node->type) {
        case NODE_PROGRAM:
            for (auto child : node->getChildren()) {
                interpret(child);
            }
            break;
            
        case NODE_STATEMENT:
            if (node->value == "use") {
                if (node->getChildCount() > 0) {
                    interpret(node->getChild(0));
                }
            } else if (node->value == "cra") {
                // 函数定义，暂时不做处理
            } else if (node->value == "del") {
                // 函数删除，暂时不做处理
            } else if (node->value == "cre") {
                // 接口创建，暂时不做处理
            }
            break;
            
        case NODE_ASSIGN:
            if (node->getChildCount() >= 2) {
                ASTNode* left = node->getChild(0);
                ASTNode* right = node->getChild(1);
                
                if (left && left->type == NODE_IDENTIFIER) {
                    GCValue value = evaluateExpression(right);
                    variables[left->value] = value;
                }
            }
            break;
            
        case NODE_CALL:
            {
                std::vector<std::string> args;
                for (size_t i = 0; i < node->getChildCount(); i++) {
                    GCValue value = evaluateExpression(node->getChild(i));
                    args.push_back(value.toString());
                }
                
                auto it = functions.find(node->value);
                if (it != functions.end()) {
                    it->second(args);
                } else {
                    std::cerr << "Error: Unknown function " << node->value << std::endl;
                }
            }
            break;
            
        case NODE_IF_STATEMENT:
            if (node->getChildCount() >= 1) {
                GCValue condition = evaluateExpression(node->getChild(0));
                if (condition.asBoolean()) {
                    // 执行if块
                    if (node->getChildCount() >= 2) {
                        interpret(node->getChild(1)); // if块
                    }
                } else {
                    // 检查是否有else块
                    if (node->getChildCount() >= 3) {
                        interpret(node->getChild(2)); // else块
                    }
                }
            }
            break;
            
        case NODE_WHILE_STATEMENT:
            if (node->getChildCount() >= 2) {
                ASTNode* condition = node->getChild(0);
                ASTNode* body = node->getChild(1);
                
                while (true) {
                    GCValue condValue = evaluateExpression(condition);
                    if (!condValue.asBoolean()) {
                        break;
                    }
                    interpret(body);
                }
            }
            break;
            
        case NODE_FOR_STATEMENT:
            if (node->getChildCount() >= 4) {
                ASTNode* init = node->getChild(0);
                ASTNode* condition = node->getChild(1);
                ASTNode* increment = node->getChild(2);
                ASTNode* body = node->getChild(3);
                
                // 初始化
                interpret(init);
                
                while (true) {
                    GCValue condValue = evaluateExpression(condition);
                    if (!condValue.asBoolean()) {
                        break;
                    }
                    interpret(body);
                    interpret(increment);
                }
            }
            break;
            
        case NODE_BREAK_STATEMENT:
            // break语句由循环结构处理
            throw std::runtime_error("break statement outside loop");
            
        case NODE_CONTINUE_STATEMENT:
            // continue语句由循环结构处理
            throw std::runtime_error("continue statement outside loop");
            
        default:
            break;
    }
}

GCValue Interpreter::evaluateExpression(ASTNode* node) {
    if (!node) return GCValue();
    
    switch (node->type) {
        case NODE_NUMBER_LITERAL:
            return GCValue(std::stod(node->value));
            
        case NODE_STRING_LITERAL:
            return GCValue(node->value);
            
        case NODE_IDENTIFIER:
            {
                auto it = variables.find(node->value);
                if (it != variables.end()) {
                    return it->second;
                }
                return GCValue(); // 返回nil
            }
            
        case NODE_OPERATOR:
            if (node->getChildCount() >= 2) {
                GCValue left = evaluateExpression(node->getChild(0));
                GCValue right = evaluateExpression(node->getChild(1));
                
                if (node->value == "+") {
                    if (left.isString() || right.isString()) {
                        return GCValue(left.toString() + right.toString());
                    } else {
                        return GCValue(left.toNumber() + right.toNumber());
                    }
                } else if (node->value == "-") {
                    return GCValue(left.toNumber() - right.toNumber());
                } else if (node->value == "*") {
                    return GCValue(left.toNumber() * right.toNumber());
                } else if (node->value == "/") {
                    double r = right.toNumber();
                    if (r != 0) {
                        return GCValue(left.toNumber() / r);
                    } else {
                        throw std::runtime_error("Division by zero");
                    }
                } else if (node->value == "%") {
                    double r = right.toNumber();
                    if (r != 0) {
                        return GCValue(fmod(left.toNumber(), r));
                    } else {
                        throw std::runtime_error("Modulo by zero");
                    }
                }
            }
            break;
            
        default:
            break;
    }
    
    return GCValue();
}

// 字节码生成器实现
Bytecode BytecodeGenerator::generate(ASTNode* ast) {
    // 重置状态
    bytecode = Bytecode();
    stringConstants.clear();
    numberConstants.clear();
    generalConstants.clear();
    functionNames.clear();
    loopStack.clear();
    
    // 生成字节码
    generateNode(ast);
    
    // 添加程序结束指令
    writer.writeOpCode(OP_HALT);
    
    return bytecode;
}

uint16_t BytecodeGenerator::addStringConstant(const std::string& str) {
    auto it = stringConstants.find(str);
    if (it != stringConstants.end()) {
        return it->second;
    }
    
    uint16_t index = static_cast<uint16_t>(bytecode.strings.size());
    bytecode.strings.push_back(str);
    stringConstants[str] = index;
    return index;
}

uint16_t BytecodeGenerator::addNumberConstant(double num) {
    auto it = numberConstants.find(num);
    if (it != numberConstants.end()) {
        return it->second;
    }
    
    uint16_t index = static_cast<uint16_t>(bytecode.numbers.size());
    bytecode.numbers.push_back(num);
    numberConstants[num] = index;
    return index;
}

uint16_t BytecodeGenerator::addGeneralConstant(const std::string& constant) {
    auto it = generalConstants.find(constant);
    if (it != generalConstants.end()) {
        return it->second;
    }
    
    uint16_t index = static_cast<uint16_t>(bytecode.constants.size());
    bytecode.constants.push_back(constant);
    generalConstants[constant] = index;
    return index;
}

uint16_t BytecodeGenerator::addFunctionName(const std::string& functionName) {
    auto it = functionNames.find(functionName);
    if (it != functionNames.end()) {
        return it->second;
    }
    
    uint16_t index = static_cast<uint16_t>(bytecode.functions.size());
    bytecode.functions.push_back(functionName);
    functionNames[functionName] = index;
    return index;
}

void BytecodeGenerator::generateNode(ASTNode* node) {
    if (!node) return;
    
    switch (node->type) {
        case NODE_PROGRAM:
            for (auto child : node->getChildren()) {
                generateNode(child);
            }
            break;
            
        case NODE_STATEMENT:
            generateStatement(node);
            break;
            
        case NODE_IF_STATEMENT:
            generateIfStatement(node);
            break;
            
        case NODE_WHILE_STATEMENT:
            generateWhileStatement(node);
            break;
            
        case NODE_FOR_STATEMENT:
            generateForStatement(node);
            break;
            
        case NODE_BREAK_STATEMENT:
            generateBreakStatement(node);
            break;
            
        case NODE_CONTINUE_STATEMENT:
            generateContinueStatement(node);
            break;
            
        case NODE_ASSIGN:
            generateAssignment(node);
            break;
            
        case NODE_CALL:
            generateCall(node);
            break;
            
        case NODE_OPERATOR:
        case NODE_NUMBER_LITERAL:
        case NODE_EXPRESSION:
        case NODE_IDENTIFIER:
            generateExpression(node);
            break;
            
        case NODE_DIRECTIVE:
            // 暂时不处理指令的字节码生成
            break;
            
        default:
            break;
    }
}

void BytecodeGenerator::generateStatement(ASTNode* node) {
    if (node->value == "use") {
        if (node->getChildCount() > 0) {
            generateNode(node->getChild(0));
        }
    } else if (node->value == "cra") {
        // 函数定义
        for (size_t i = 1; i < node->getChildCount(); i++) {
            generateNode(node->getChild(i));
        }
    } else if (node->value == "cre") {
        // 接口创建 - 暂时不处理
    } else if (node->value == "del") {
        // 接口删除 - 暂时不处理
    }
}

void BytecodeGenerator::generateIfStatement(ASTNode* node) {
    // 节点结构: NODE_IF_STATEMENT -> [条件表达式, if块, else块(可选)]
    if (node->getChildCount() < 2) return;
    
    // 生成条件表达式
    generateNode(node->getChild(0));
    
    // 如果条件为假，跳转到else块或if语句结束的位置
    size_t elseJumpPos = writer.getPosition();
    writer.writeOpCode(OP_JMP_IF_FALSE);
    writer.writePositionPlaceholder();
    
    // 生成if块
    generateNode(node->getChild(1));
    
    // 如果有else块，需要跳过else块
    if (node->getChildCount() >= 3) {
        // 跳转到if语句结束的位置
        size_t endJumpPos = writer.getPosition();
        writer.writeOpCode(OP_JMP);
        writer.writePositionPlaceholder();
        
        // 更新else跳转位置
        size_t elseStartPos = writer.getPosition();
        writer.updatePositionPlaceholder(elseJumpPos, elseStartPos);
        
        // 生成else块
        generateNode(node->getChild(2));
        
        // 更新结束跳转位置
        size_t endPos = writer.getPosition();
        writer.updatePositionPlaceholder(endJumpPos, endPos);
    } else {
        // 没有else块，直接更新else跳转位置
        size_t endPos = writer.getPosition();
        writer.updatePositionPlaceholder(elseJumpPos, endPos);
    }
}

void BytecodeGenerator::generateWhileStatement(ASTNode* node) {
    // 节点结构: NODE_WHILE_STATEMENT -> [条件表达式, 循环体]
    if (node->getChildCount() < 2) return;
    
    // 记录循环开始的位置
    size_t loopStartPos = writer.getPosition();
    
    // 生成条件表达式
    generateNode(node->getChild(0));
    
    // 如果条件为假，跳转到循环结束的位置
    size_t loopEndJumpPos = writer.getPosition();
    writer.writeOpCode(OP_JMP_IF_FALSE);
    writer.writePositionPlaceholder();
    
    // 将循环信息压入循环栈
    loopStack.push_back(std::make_pair(loopStartPos, loopEndJumpPos));
    
    // 生成循环体
    generateNode(node->getChild(1));
    
    // 跳回到循环开始的位置
    writer.writeOpCode(OP_JMP);
    writer.writeInt(loopStartPos);
    
    // 更新循环结束跳转的位置
    size_t loopEndPos = writer.getPosition();
    writer.updatePositionPlaceholder(loopEndJumpPos, loopEndPos);
    
    // 弹出循环栈
    loopStack.pop_back();
}

void BytecodeGenerator::generateForStatement(ASTNode* node) {
    // 节点结构: NODE_FOR_STATEMENT -> [初始化表达式, 条件表达式, 迭代表达式, 循环体]
    if (node->getChildCount() < 4) return;
    
    // 生成初始化表达式
    generateNode(node->getChild(0));
    
    // 记录循环开始的位置
    size_t loopStartPos = writer.getPosition();
    
    // 生成条件表达式
    generateNode(node->getChild(1));
    
    // 如果条件为假，跳转到循环结束的位置
    size_t loopEndJumpPos = writer.getPosition();
    writer.writeOpCode(OP_JMP_IF_FALSE);
    writer.writePositionPlaceholder();
    
    // 记录循环结束位置并压入循环栈
    size_t loopEndPosPlaceholder = writer.getPosition();
    writer.writeOpCode(OP_NOP); // 占位符，稍后会被实际的循环结束位置替换
    writer.writePositionPlaceholder();
    
    // 将循环信息压入循环栈
    loopStack.push_back(std::make_pair(loopStartPos, 0)); // 0是占位符，稍后会更新
    
    // 生成循环体
    generateNode(node->getChild(3));
    
    // 生成迭代表达式
    generateNode(node->getChild(2));
    
    // 跳回到循环开始的位置
    writer.writeOpCode(OP_JMP);
    writer.writeInt(loopStartPos);
    
    // 更新循环结束跳转的位置
    size_t loopEndPos = writer.getPosition();
    writer.updatePositionPlaceholder(loopEndJumpPos, loopEndPos);
    
    // 弹出循环栈
    loopStack.pop_back();
}

void BytecodeGenerator::generateBreakStatement(ASTNode* node) {
    // 生成OP_BREAK opcode
    writer.writeOpCode(OP_BREAK);
}

void BytecodeGenerator::generateContinueStatement(ASTNode* node) {
    // 生成OP_CONTINUE opcode
    writer.writeOpCode(OP_CONTINUE);
}

void BytecodeGenerator::generateAssignment(ASTNode* node) {
    // 节点结构: NODE_ASSIGN -> [左值标识符, 右值表达式]
    if (node->getChildCount() < 2) return;
    
    // 生成右值表达式
    generateNode(node->getChild(1));
    
    // 存储到变量
    ASTNode* left = node->getChild(0);
    if (left && left->type == NODE_IDENTIFIER) {
        uint16_t varIndex = addGeneralConstant(left->value);
        writer.writeOpCode(OP_STORE);
        writer.writeShort(varIndex);
    }
}

void BytecodeGenerator::generateCall(ASTNode* node) {
    // 生成所有参数
    for (size_t i = 0; i < node->getChildCount(); i++) {
        generateNode(node->getChild(i));
    }
    
    // 生成函数调用
    uint16_t funcIndex = addFunctionName(node->value);
    writer.writeOpCode(OP_CALL);
    writer.writeShort(funcIndex);
    writer.writeByte(static_cast<uint8_t>(node->getChildCount())); // 参数数量
}

void BytecodeGenerator::generateExpression(ASTNode* node) {
    if (!node) return;
    
    switch (node->type) {
        case NODE_NUMBER_LITERAL:
            {
                double value = std::stod(node->value);
                uint16_t index = addNumberConstant(value);
                writer.writeOpCode(OP_PUSH_NUM);
                writer.writeShort(index);
            }
            break;
            
        case NODE_STRING_LITERAL:
            {
                uint16_t index = addStringConstant(node->value);
                writer.writeOpCode(OP_PUSH_STR);
                writer.writeShort(index);
            }
            break;
            
        case NODE_IDENTIFIER:
            {
                uint16_t varIndex = addGeneralConstant(node->value);
                writer.writeOpCode(OP_LOAD);
                writer.writeShort(varIndex);
            }
            break;
            
        case NODE_OPERATOR:
            if (node->getChildCount() >= 2) {
                // 生成左右操作数
                generateNode(node->getChild(0));
                generateNode(node->getChild(1));
                
                // 生成操作码
                if (node->value == "+") {
                    writer.writeOpCode(OP_ADD);
                } else if (node->value == "-") {
                    writer.writeOpCode(OP_SUB);
                } else if (node->value == "*") {
                    writer.writeOpCode(OP_MUL);
                } else if (node->value == "/") {
                    writer.writeOpCode(OP_DIV);
                } else if (node->value == "%") {
                    writer.writeOpCode(OP_MOD);
                }
            }
            break;
            
        case NODE_EXPRESSION:
            // 递归生成子表达式
            for (auto child : node->getChildren()) {
                generateNode(child);
            }
            break;
            
        default:
            break;
    }
}

// 虚拟机实现
void BytecodeVM::initBaselib() {
    // 注册基础库函数
    functions["System.Output.Print"] = [](const std::vector<std::string>& args) {
        for (const auto& arg : args) {
            std::cout << arg;
        }
        std::cout << std::endl;
    };
    
    functions["System.Output.Println"] = [](const std::vector<std::string>& args) {
        for (const auto& arg : args) {
            std::cout << arg;
        }
        std::cout << std::endl;
    };
    
    functions["System.Input.Read"] = [](const std::vector<std::string>& args) {
        std::string input;
        std::cin >> input;
        std::cout << input;
    };
    
    functions["System.Input.ReadLine"] = [](const std::vector<std::string>& args) {
        std::string input;
        std::getline(std::cin, input);
        std::cout << input;
    };
    
    // 添加时间相关函数
    functions["System.Time.Now"] = [](const std::vector<std::string>& args) {
        auto now = std::chrono::high_resolution_clock::now();
        auto duration = now.time_since_epoch();
        auto milliseconds = std::chrono::duration_cast<std::chrono::milliseconds>(duration).count();
        std::cout << milliseconds;
    };
    
    // 数学函数
    functions["System.Math.Add"] = [](const std::vector<std::string>& args) {
        if (args.size() >= 2) {
            double a = std::stod(args[0]);
            double b = std::stod(args[1]);
            std::cout << (a + b);
        }
    };
    
    functions["System.Math.Subtract"] = [](const std::vector<std::string>& args) {
        if (args.size() >= 2) {
            double a = std::stod(args[0]);
            double b = std::stod(args[1]);
            std::cout << (a - b);
        }
    };
    
    functions["System.Math.Multiply"] = [](const std::vector<std::string>& args) {
        if (args.size() >= 2) {
            double a = std::stod(args[0]);
            double b = std::stod(args[1]);
            std::cout << (a * b);
        }
    };
    
    functions["System.Math.Divide"] = [](const std::vector<std::string>& args) {
        if (args.size() >= 2) {
            double a = std::stod(args[0]);
            double b = std::stod(args[1]);
            if (b != 0) {
                std::cout << (a / b);
            } else {
                std::cout << "Error: Division by zero";
            }
        }
    };
    
    functions["System.Math.Modulo"] = [](const std::vector<std::string>& args) {
        if (args.size() >= 2) {
            double a = std::stod(args[0]);
            double b = std::stod(args[1]);
            if (b != 0) {
                std::cout << fmod(a, b);
            } else {
                std::cout << "Error: Modulo by zero";
            }
        }
    };
}

GCValue BytecodeVM::pop() {
    if (stack.empty()) {
        throw std::runtime_error("Stack underflow");
    }
    GCValue value = stack.back();
    stack.pop_back();
    return value;
}

void BytecodeVM::push(const GCValue& value) {
    stack.push_back(value);
}

void BytecodeVM::execute(const Bytecode& bytecode) {
    // 初始化虚拟机状态
    stack.clear();
    variables.clear();
    programCounter = 0;
    initBaselib();
    
    // 注册VM到GCValue
    GCValue::registerVM(stack, variables);
    
    // 执行字节码
    int instructionCount = 0;
    while (programCounter < bytecode.code.size()) {
        executeInstruction(bytecode);
        instructionCount++;
        
        // 每执行1000条指令进行一次垃圾回收
        if (instructionCount % 1000 == 0) {
            GCValue::collectGarbage();
        }
    }
    
    // 注销VM
    GCValue::unregisterVM();
}

void BytecodeVM::executeInstruction(const Bytecode& bytecode) {
    if (programCounter >= bytecode.code.size()) {
        throw std::runtime_error("Program counter out of bounds");
    }
    
    OpCode op = static_cast<OpCode>(bytecode.code[programCounter++]);
    
    switch (op) {
        case OP_NOP:
            break;
            
        case OP_PUSH_NUM:
            {
                uint16_t index = (bytecode.code[programCounter] << 8) | bytecode.code[programCounter + 1];
                programCounter += 2;
                if (index < bytecode.numbers.size()) {
                    push(GCValue(bytecode.numbers[index]));
                }
            }
            break;
            
        case OP_PUSH_STR:
            {
                uint16_t index = (bytecode.code[programCounter] << 8) | bytecode.code[programCounter + 1];
                programCounter += 2;
                if (index < bytecode.strings.size()) {
                    push(GCValue(bytecode.strings[index]));
                }
            }
            break;
            
        case OP_PUSH_CONST:
            {
                uint16_t index = (bytecode.code[programCounter] << 8) | bytecode.code[programCounter + 1];
                programCounter += 2;
                if (index < bytecode.constants.size()) {
                    push(GCValue(bytecode.constants[index]));
                }
            }
            break;
            
        case OP_POP:
            pop();
            break;
            
        case OP_ADD:
            {
                GCValue b = pop();
                GCValue a = pop();
                if (a.isString() || b.isString()) {
                    push(GCValue(a.toString() + b.toString()));
                } else {
                    push(GCValue(a.toNumber() + b.toNumber()));
                }
            }
            break;
            
        case OP_SUB:
            {
                GCValue b = pop();
                GCValue a = pop();
                push(GCValue(a.toNumber() - b.toNumber()));
            }
            break;
            
        case OP_MUL:
            {
                GCValue b = pop();
                GCValue a = pop();
                push(GCValue(a.toNumber() * b.toNumber()));
            }
            break;
            
        case OP_DIV:
            {
                GCValue b = pop();
                GCValue a = pop();
                double divisor = b.toNumber();
                if (divisor != 0) {
                    push(GCValue(a.toNumber() / divisor));
                } else {
                    throw std::runtime_error("Division by zero");
                }
            }
            break;
            
        case OP_MOD:
            {
                GCValue b = pop();
                GCValue a = pop();
                double divisor = b.toNumber();
                if (divisor != 0) {
                    push(GCValue(fmod(a.toNumber(), divisor)));
                } else {
                    throw std::runtime_error("Modulo by zero");
                }
            }
            break;
            
        case OP_CALL:
            {
                uint16_t funcIndex = (bytecode.code[programCounter] << 8) | bytecode.code[programCounter + 1];
                programCounter += 2;
                uint8_t argCount = bytecode.code[programCounter++];
                
                if (funcIndex < bytecode.functions.size()) {
                    std::string funcName = bytecode.functions[funcIndex];
                    
                    // 获取参数
                    std::vector<std::string> args;
                    for (int i = argCount - 1; i >= 0; i--) {
                        Value arg = stack[stack.size() - 1 - i];
                        args.push_back(arg.toString());
                    }
                    
                    // 弹出参数
                    for (uint8_t i = 0; i < argCount; i++) {
                        pop();
                    }
                    
                    // 调用函数
                    auto it = functions.find(funcName);
                    if (it != functions.end()) {
                        it->second(args);
                    } else {
                        std::cerr << "Error: Unknown function " << funcName << std::endl;
                    }
                }
            }
            break;
            
        case OP_JMP:
            {
                uint32_t target = (bytecode.code[programCounter] << 24) |
                                 (bytecode.code[programCounter + 1] << 16) |
                                 (bytecode.code[programCounter + 2] << 8) |
                                 bytecode.code[programCounter + 3];
                programCounter = target;
            }
            break;
            
        case OP_JMP_IF_FALSE:
            {
                uint32_t target = (bytecode.code[programCounter] << 24) |
                                 (bytecode.code[programCounter + 1] << 16) |
                                 (bytecode.code[programCounter + 2] << 8) |
                                 bytecode.code[programCounter + 3];
                programCounter += 4;
                
                GCValue condition = pop();
                if (!condition.asBoolean()) {
                    programCounter = target;
                }
            }
            break;
            
        case OP_JMP_IF_TRUE:
            {
                uint32_t target = (bytecode.code[programCounter] << 24) |
                                 (bytecode.code[programCounter + 1] << 16) |
                                 (bytecode.code[programCounter + 2] << 8) |
                                 bytecode.code[programCounter + 3];
                programCounter += 4;
                
                GCValue condition = pop();
                if (condition.asBoolean()) {
                    programCounter = target;
                }
            }
            break;
            
        case OP_LOAD:
            {
                uint16_t varIndex = (bytecode.code[programCounter] << 8) | bytecode.code[programCounter + 1];
                programCounter += 2;
                
                if (varIndex < bytecode.constants.size()) {
                    std::string varName = bytecode.constants[varIndex];
                    auto it = variables.find(varName);
                    if (it != variables.end()) {
                        push(it->second);
                    } else {
                        push(GCValue()); // nil
                    }
                }
            }
            break;
            
        case OP_STORE:
            {
                uint16_t varIndex = (bytecode.code[programCounter] << 8) | bytecode.code[programCounter + 1];
                programCounter += 2;
                
                if (varIndex < bytecode.constants.size()) {
                    std::string varName = bytecode.constants[varIndex];
                    GCValue value = pop();
                    variables[varName] = value;
                }
            }
            break;
            
        case OP_RET:
            // 函数返回，对于简单实现，我们不需要特殊处理
            break;
            
        case OP_HALT:
            programCounter = static_cast<uint32_t>(bytecode.code.size()); // 结束执行
            break;
            
        default:
            throw std::runtime_error("Unknown opcode: " + std::to_string(op));
    }
}

// 编译字节码为可执行文件
void compileBytecodeToExe(const Bytecode& bytecode, const std::string& outputFilename) {
    // 创建C++源文件
    std::string cppFilename = outputFilename + ".cpp";
    std::ofstream cppFile(cppFilename);
    
    if (!cppFile.is_open()) {
        throw std::runtime_error("Could not create temporary C++ file");
    }
    
    // 写入C++代码头
    cppFile << "#include <iostream>\n";
    cppFile << "#include <string>\n";
    cppFile << "#include <vector>\n";
    cppFile << "#include <map>\n";
    cppFile << "#include <functional>\n";
    cppFile << "#include <sstream>\n";
    cppFile << "#include <chrono>\n";
    cppFile << "#include <cmath>\n";
    cppFile << "\n";
    cppFile << "// 简化的值类型系统\n";
    cppFile << "class Value {\n";
    cppFile << "public:\n";
    cppFile << "    enum class Type { NUMBER, STRING, BOOLEAN, NIL };\n";
    cppFile << "    \n";
    cppFile << "    Value() : type_(Type::NIL) {}\n";
    cppFile << "    Value(double n) : type_(Type::NUMBER), number_(n) {}\n";
    cppFile << "    Value(const std::string& s) : type_(Type::STRING), string_(s) {}\n";
    cppFile << "    Value(bool b) : type_(Type::BOOLEAN), boolean_(b) {}\n";
    cppFile << "    \n";
    cppFile << "    Type getType() const { return type_; }\n";
    cppFile << "    bool isNumber() const { return type_ == Type::NUMBER; }\n";
    cppFile << "    bool isString() const { return type_ == Type::STRING; }\n";
    cppFile << "    bool isBoolean() const { return type_ == Type::BOOLEAN; }\n";
    cppFile << "    bool isNil() const { return type_ == Type::NIL; }\n";
    cppFile << "    \n";
    cppFile << "    double asNumber() const { return isNumber() ? number_ : 0.0; }\n";
    cppFile << "    std::string asString() const { return isString() ? string_ : \"\"; }\n";
    cppFile << "    bool asBoolean() const { return isBoolean() ? boolean_ : false; }\n";
    cppFile << "    \n";
    cppFile << "    std::string toString() const {\n";
    cppFile << "        switch (type_) {\n";
    cppFile << "            case Type::NUMBER: return std::to_string(number_);\n";
    cppFile << "            case Type::STRING: return string_;\n";
    cppFile << "            case Type::BOOLEAN: return boolean_ ? \"true\" : \"false\";\n";
    cppFile << "            case Type::NIL: return \"nil\";\n";
    cppFile << "            default: return \"unknown\";\n";
    cppFile << "        }\n";
    cppFile << "    }\n";
    cppFile << "    \n";
    cppFile << "    double toNumber() const {\n";
    cppFile << "        switch (type_) {\n";
    cppFile << "            case Type::NUMBER: return number_;\n";
    cppFile << "            case Type::STRING: return std::stod(string_);\n";
    cppFile << "            case Type::BOOLEAN: return boolean_ ? 1.0 : 0.0;\n";
    cppFile << "            case Type::NIL: return 0.0;\n";
    cppFile << "            default: return 0.0;\n";
    cppFile << "        }\n";
    cppFile << "    }\n";
    cppFile << "\n";
    cppFile << "private:\n";
    cppFile << "    Type type_;\n";
    cppFile << "    union {\n";
    cppFile << "        double number_;\n";
    cppFile << "        bool boolean_;\n";
    cppFile << "    };\n";
    cppFile << "    std::string string_;\n";
    cppFile << "};\n";
    cppFile << "\n";
    cppFile << "// 主函数\n";
    cppFile << "int main() {\n";
    cppFile << "    // TODO: 实现字节码执行逻辑\n";
    cppFile << "    std::cout << \"Hello from compiled Slime program!\" << std::endl;\n";
    cppFile << "    return 0;\n";
    cppFile << "}\n";
    
    cppFile.close();
    
    // 使用系统编译器编译C++文件
    std::string compileCmd;
    
    // 尝试使用MinGW编译器
    compileCmd = "g++ -o \"" + outputFilename + "\" \"" + cppFilename + "\" -std=c++11";
    if (std::system(compileCmd.c_str()) != 0) {
        // 尝试使用MSVC编译器
        compileCmd = "cl /EHsc /Fe\"" + outputFilename + "\" \"" + cppFilename + "\"";
        if (std::system(compileCmd.c_str()) != 0) {
            // 删除临时文件
            std::remove(cppFilename.c_str());
            throw std::runtime_error("Could not compile to exe: No suitable compiler found");
        }
    }
    
    // 删除临时文件
    std::remove(cppFilename.c_str());
    
    std::cout << "Executable saved to " << outputFilename << std::endl;
}

// 主函数
int main(int argc, char* argv[]) {
    if (argc < 2) {
        std::cerr << "Usage: " << argv[0] << " <filename> [--compile <output.btc>]" << std::endl;
        std::cerr << "Options:\n";
        std::cerr << "  --compile <output.btc>     Compile to bytecode file instead of executing\n" << std::endl;
        std::cerr << "  --run <bytecode.btc>       Run bytecode file\n" << std::endl;
        std::cerr << "  --compile-to-exe <input> <output.exe>  Compile to executable file\n" << std::endl;
        return 1;
    }

    std::string filename = argv[1];
    
    // 检查是否是运行字节码文件
    if (filename == "--run" && argc == 3) {
        std::string btcFilename = argv[2];
        try {
            Bytecode bytecode = loadBytecodeFromFile(btcFilename);
            BytecodeVM vm;
            vm.execute(bytecode);
        } catch (const std::exception& e) {
            std::cerr << "Error: " << e.what() << std::endl;
            return 1;
        }
        return 0;
    }
    
    // 检查是否是编译选项
    if (filename == "--compile" && argc == 4) {
        std::string inputFilename = argv[2];
        std::string outputFilename = argv[3];
        
        std::ifstream file(inputFilename);
        if (!file.is_open()) {
            std::cerr << "Error: Could not open file " << inputFilename << std::endl;
            return 1;
        }
        
        std::string content((std::istreambuf_iterator<char>(file)),
                            (std::istreambuf_iterator<char>()));
        file.close();
        
        try {
            // 解析代码生成AST
            Lexer lexer(content);
            Parser parser(lexer);
            ASTNode* ast = parser.parse();
            
            try {
                // 生成字节码
                BytecodeGenerator generator;
                Bytecode bytecode = generator.generate(ast);
                
                // 保存字节码到文件
                saveBytecodeToFile(bytecode, outputFilename);
            } catch (const std::exception&) {
                delete ast;
                throw;
            }
            
            delete ast;
        } catch (const std::exception& e) {
            std::cerr << "Error: " << e.what() << std::endl;
            return 1;
        }
        return 0;
    }
    
    // 检查是否是编译为exe选项
    if (filename == "--compile-to-exe" && argc == 4) {
        std::string inputFilename = argv[2];
        std::string outputFilename = argv[3];
        
        std::ifstream file(inputFilename);
        if (!file.is_open()) {
            std::cerr << "Error: Could not open file " << inputFilename << std::endl;
            return 1;
        }
        
        std::string content((std::istreambuf_iterator<char>(file)),
                            (std::istreambuf_iterator<char>()));
        file.close();
        
        try {
            // 解析代码生成AST
            Lexer lexer(content);
            Parser parser(lexer);
            ASTNode* ast = parser.parse();
            
            try {
                // 生成字节码
                BytecodeGenerator generator;
                Bytecode bytecode = generator.generate(ast);
                
                // 编译为exe文件
                compileBytecodeToExe(bytecode, outputFilename);
            } catch (const std::exception&) {
                delete ast;
                throw;
            }
            
            delete ast;
        } catch (const std::exception& e) {
            std::cerr << "Error: " << e.what() << std::endl;
            return 1;
        }
        return 0;
    }
    
    // 默认直接执行代码
    std::ifstream file(filename);
    if (!file.is_open()) {
        std::cerr << "Error: Could not open file " << filename << std::endl;
        return 1;
    }

    std::string content((std::istreambuf_iterator<char>(file)),
                        (std::istreambuf_iterator<char>()));
    file.close();

    Interpreter interpreter;
    try {
        interpreter.execute(content);
        // 执行垃圾回收
        GCValue::collectGarbage();
    } catch (const std::exception& e) {
        std::cerr << "Error: " << e.what() << std::endl;
        return 1;
    }

    return 0;
}