Make the system act as a mcp server.
I want to be able to connect claude code and hermes agent to it.
Write a full document on how to install it as a mcp server in these environments:
- claude code
- hermes agent
- other mcp compatible tools


### file tools extension
Can you add to the mcp server a file tools extension that will allow me to manage files in the system?
- list files
- read files
- write files
- delete files
- create directories
- delete directories
- move files
- rename
- copy files
- get file info
- get directory info
This mcp would only have access to the folders defined in the configuration file (CLAUDE_CLI_WORKSPACE_ROOTS)
As we will add more providers (as with minimax), we should make this mcp server extensible so that we can add more tools to it in the future. And it therfor makes sense to rename that parameter to something more generic like WORKSPACE_ROOTS