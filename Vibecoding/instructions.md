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