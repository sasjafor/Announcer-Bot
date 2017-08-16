FROM node:boron

# Create app directory
WORKDIR /usr/src/app

# Install app dependencies
COPY package.json .

RUN npm install discord.io winston --save

# Copy source
COPY . .

EXPOSE 8080
CMD [ "npm", "start" ]
