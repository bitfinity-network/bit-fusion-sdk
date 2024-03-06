FROM node:latest

WORKDIR /usr/src
RUN git clone https://github.com/bezrazli4n0/ordinals-explorer
WORKDIR /usr/src/ordinals-explorer
RUN npm install
RUN npm run build

ENTRYPOINT [ "npm", "start" ]
