#ifndef HTML_PARSER_H
#define HTML_PARSER_H

#include <string>
#include <vector>
#include "Token.h"
#include "DOMNode.h"
#include "Tokenizer.h"
#include <stack>
enum class ParserMode {
    NORMAL,
    IN_SCRIPT,
    IN_STYLE
};

class HtmlParser {
public:
    // parse the HTML string, return a pointer to the root DOM node
    DOMNode* parse(const std::string& html);

private:
    // Helper: build the tree using a stack-based approach
    DOMNode* buildDOM(const std::vector<Token>& tokens);

    // Additional methods for advanced logic
    void handleStartTag(Token& token, std::stack<DOMNode*>& nodeStack);
    void handleEndTag(Token& token, std::stack<DOMNode*>& nodeStack);
    void handleSelfClosingTag(Token& token, std::stack<DOMNode*>& nodeStack);
    void handleTextOrComment(Token& token, std::stack<DOMNode*>& nodeStack);

    ParserMode mode = ParserMode::NORMAL;  // Tracks current parser mode
};

#endif // HTML_PARSER_H
