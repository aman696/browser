#ifndef TOKEN_H
#define TOKEN_H

#include <string>
#include <map>

// Different kinds of tokens our HTML parser will recognize
enum class TokenType {
    DOCTYPE,
    START_TAG,
    END_TAG,
    SELF_CLOSING_TAG,
    COMMENT,
    TEXT
};

struct Token {
    TokenType type;
    std::string tagName;                  // e.g. "html", "body", "p", etc.
    std::map<std::string, std::string> attributes; // For START_TAG or SELF_CLOSING
    std::string textContent;              // For TEXT or COMMENT
};

#endif // TOKEN_H
