Build a web application that will serve as a platform to manage and organize all my data, files, ideas.
The platform will be called Baitler (Butler + AI). Site: Baitler.com
It will serve as a personal assistant and data organizer.

## Tech stack:

### Frontend:
- React/vite
- TailwindCSS
- TypeScript

### Backend:
- Rust based api
- SurrealDB for storage/database

### Mobile
- iOS and Android apps

### Development tools
- scripts folder with
    - dev run scripts to run frontend and backend simultaneously 
    - build scripts to build frontend and backend

## Features:
- Base portal where all functions come together
- User management/integration with oauth2 (google, github, etc.)
- File storage and management
- Idea management and organization
- Data visualization and analytics
- AI LLM integration for data analysis and insights
    - external providers (openai, anthropic, openrouter,fal.ai, etc.)
    - multi model support
    - multi modal (text, image, video, audio)
- Html document editor and management
- Pdf export
- MS office export (word, excel, powerpoint)
- Markdown support 


Use the git repo at: https://github.com/bartbeecoders/baitler.git


## Base agentic idea

I want to use MCP (Model Context Protocol) to enable agentic capabilities in the application.
The idea is that I will give claude code instructions to document, illustrate certain projects I'm working on. This documentation wil be stored in Baitler. So Baitler will become my personal knowledge base and assistant.
Through the MCP server, the ai agents (claude code, grok code, hermes agent etc.) will be able to:
- organse the knowledge base
- make web pages/markdown/pdf/office documents so the knowledge is human readable and exportable
- access the data in Baitler and use it to answer questions, generate content, and perform other tasks.

Work this idea out completely and add this to the plan.md.

## Web page hosting

I want to be able to host web pages on Baitler. The web pages should be able to be created from markdown, html, or other formats. The web pages should be able to be published and shared with others. The web pages should be able to be accessed via a url. The web pages should be able to be edited and updated. The web pages should be able to be deleted. The web pages should be able to be organized in folders. The web pages should be able to be searched. The web pages should be able to be filtered by type, date, author, etc. The web pages should be able to be shared with others. The web pages should be able to be published and shared with others. The web pages should be able to be accessed via a url. The web pages should be able to be edited and updated. The web pages should be able to be deleted. The web pages should be able to be organized in folders. The web pages should be able to be searched. The web pages should be able to be filtered by type, date, author, etc.

Work this idea out completely and add this to the plan.md.

## document, information, web organisation improvements

Add metadata (tags) to documents, information, and web pages to improve organization and searchability.
Add mindmap capability to organize ideas and information visually.
Add draw.io integration for diagram creation and management.

Work this idea out completely and add this to the plan.md.


## Claude code cli integration

Integrate Claude Code CLI so a user can invoke it from within the application to perform tasks.
Build a cli wrapper that can be used to invoke claude code commands from within the application.

Work this idea out completely and add this to the plan.md.

## add minimax 

Add minimax (Minimax-M3) as an AI agent provider. see https://platform.minimax.io/docs/token-plan/other-tools for docs.

Add a selection to the agent panel to choose the AI agent provider. (Claude code or Minimax-M3)

Put the agent page as a right hand pane on each of the pages (Files,Ideas, Documents, Web Pages, etc.)
This way we will let the agent interact with the content in the current page.


## Superpage

What about combining documents, ideas, web pages, diagrams, mindmaps and other content into a superpage? This would allow the agent to interact with all of this content in one place.
1 big page, where the user can place all the content they want to interact with.
./Scc   


## Minimalistic design

We are onto something.
Lets go back to a simple full page canvas for the agent to work on.
Just some simple menu items to interact with the canvas:
- add a text part
- add a image part
- add a code part
- add a file part
- add a web page part
- add a mindmap part
- add a diagram part

AI agent should be able to interact with all of these parts. Could be a pop up pane that appears when the user clicks on a part or in the canvas.

Free draggable parts on the canvas.
Free drag and drop parts on the canvas.
Auto save the canvas.

## redesign


