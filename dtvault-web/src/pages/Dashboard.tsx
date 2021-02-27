import React from 'react';
import { Center, Container, Image } from '@chakra-ui/react';

function Dashboard() {
    return (
        <Container maxW="container.lg" marginTop="1rem">
            <Center>
                <Image src="/dashboard.png" boxSize="320px" />
            </Center>
        </Container>
    );
}

export default Dashboard;
