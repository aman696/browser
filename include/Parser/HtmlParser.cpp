#include "HtmlParser.h"
#include <stack>
#include <iostream>

DOMNode* HtmlParser::parse(const std::string& html) {
    Tokenizer tokenizer;
    std::vector<Token> tokens = tokenizer.tokenize(html);
    return buildDOM(tokens);
}

DOMNode* HtmlParser::buildDOM(const std::vector<Token>& tokens) {
    // Create a root DOCUMENT node
    DOMNode* root = new DOMNode(NodeType::DOCUMENT);
    root->tagName = "document";

    std::stack<DOMNode*> nodeStack;
    nodeStack.push(root);

    // Set default mode
    mode = ParserMode::NORMAL;

    for (size_t i = 0; i < tokens.size(); i++) {
        Token token = tokens[i];

        switch (token.type) {
        case TokenType::START_TAG:
            handleStartTag(token, nodeStack);
            break;

        case TokenType::END_TAG:
            handleEndTag(token, nodeStack);
            break;

        case TokenType::SELF_CLOSING_TAG:
            handleSelfClosingTag(token, nodeStack);
            break;

        case TokenType::COMMENT:
        case TokenType::TEXT:
            handleTextOrComment(token, nodeStack);
            break;

        case TokenType::DOCTYPE:
            // ignore or store in the root, your call
            break;
        }
    }

    // pop leftover nodes if not closed
    while (nodeStack.size() > 1) {
        std::cerr << "[Warning] Auto-closing unclosed <"
                  << nodeStack.top()->tagName << ">\n";
        nodeStack.pop();
    }

    return root;
}

/**
 * Handle a START_TAG token
 */
void HtmlParser::handleStartTag(Token& token, std::stack<DOMNode*>& nodeStack) {
    // create a new element node
    DOMNode* elem = new DOMNode(NodeType::ELEMENT);
    elem->tagName = token.tagName;
    elem->attributes = token.attributes;

    // attach to current parent
    nodeStack.top()->appendChild(elem);

    // Check for special tags
    if (token.tagName == "script") {
        mode = ParserMode::IN_SCRIPT;
    } else if (token.tagName == "style") {
        mode = ParserMode::IN_STYLE;
    }

    // push onto stack unless it is a known 'void' element like <br>, <meta>, etc.
    // We'll assume script/style have content, so we definitely push them
    nodeStack.push(elem);
}

/**
 * Handle an END_TAG token with error recovery
 */
void HtmlParser::handleEndTag(Token& token, std::stack<DOMNode*>& nodeStack) {
    std::string closeTag = token.tagName;

    // If we're in script mode, check if we're closing script
    if (mode == ParserMode::IN_SCRIPT && closeTag == "script") {
        // matched </script>
        mode = ParserMode::NORMAL;
    }
    // same for style
    else if (mode == ParserMode::IN_STYLE && closeTag == "style") {
        mode = ParserMode::NORMAL;
    }

    // Attempt to find a matching start tag on the stack
    while (!nodeStack.empty() && nodeStack.top()->tagName != closeTag) {
        std::cerr << "[Warning] Mismatched end tag </" << closeTag
                  << ">, auto-closing <" << nodeStack.top()->tagName << ">\n";
        nodeStack.pop(); // auto-close
    }
    // If found matching
    if (!nodeStack.empty()) {
        nodeStack.pop(); // pop the matching
    } else {
        // no matching start tag was found
        std::cerr << "[Warning] Orphan end tag </" << closeTag << ">\n";
        // we do nothing else
    }
}

/**
 * Handle SELF_CLOSING_TAG
 */
void HtmlParser::handleSelfClosingTag(Token& token, std::stack<DOMNode*>& nodeStack) {
    // e.g. <img />, <br />
    DOMNode* elem = new DOMNode(NodeType::ELEMENT);
    elem->tagName = token.tagName;
    elem->attributes = token.attributes;
    nodeStack.top()->appendChild(elem);

    // if it's script or style self closing -> unusual, but let's handle
    if (elem->tagName == "script") {
        // Possibly do something special, but script typically not self-closed
    }
    if (elem->tagName == "style") {
        // Similarly
    }
}

/**
 * Handle TEXT or COMMENT token
 * Note: if we are in script/style mode, treat everything as text
 */
void HtmlParser::handleTextOrComment(Token& token, std::stack<DOMNode*>& nodeStack) {
    DOMNode* node;
    if (token.type == TokenType::COMMENT) {
        node = new DOMNode(NodeType::COMMENT);
        node->textContent = token.textContent;
    } else {
        // TEXT
        node = new DOMNode(NodeType::TEXT);
        node->textContent = token.textContent;
    }

    // if we're in script or style mode, we do not interpret <...> as tags
    // but your Tokenizer is currently always splitting on <, so a robust approach might
    // require changes to the Tokenizer. For demonstration, we keep it simple.
    nodeStack.top()->appendChild(node);
}
