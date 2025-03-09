#ifndef RENDERER_H
#define RENDERER_H

#include "DOMNode.h"

class Renderer {
public:
    Renderer();
    ~Renderer();

    void render(DOMNode* root); // Draws the webpage
};

#endif // RENDERER_H
