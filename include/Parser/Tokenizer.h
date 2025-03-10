#ifndef TOKENIZER_H
#define TOKENIZER_H

#include <string>
#include <vector>
#include "Token.h"

class Tokenizer {
public:
    // Takes raw HTML string, returns a vector of tokens
    std::vector<Token> tokenize(const std::string& html);

private:
    // Helper methods
    bool isAlpha(char c);
    bool isSpace(char c);
};

#endif // TOKENIZER_H
