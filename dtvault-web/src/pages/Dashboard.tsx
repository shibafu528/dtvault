import React from 'react';
import { Center, Container, Image, Box, Button } from '@chakra-ui/react';
import { Link } from 'react-router-dom';

function Dashboard() {
    return (
        <Container maxW="container.lg" mt="1rem">
            <Box>
                <Center>
                    <Button as={Link} colorScheme="blue" to="/programs">
                        番組一覧
                    </Button>
                </Center>
            </Box>
            <Center mt="2rem">
                <Image src="/dashboard.png" boxSize="320px" />
            </Center>
        </Container>
    );
}

export default Dashboard;
